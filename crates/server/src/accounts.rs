//! Accounts and sign-in sessions (X-01, X-02, X-03, X-05, X-06). Pure logic over [`Db`]: no HTTP here.
//!
//! Passwords are hashed with argon2id. A session is a random token whose SHA-256 is stored, so a copy of the
//! database cannot be used to sign in. Sign-in is rate limited per account and per address with backoff.
//! Callers pass the current time (`now_ms`) so everything is testable with a fake clock.

use std::fmt;

use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::{Algorithm, Argon2, Params, Version};
use sha2::{Digest, Sha256};

use crate::db::{Attempt, Db, DbError, Role, User};

pub const USERNAME_MIN: usize = 3;
pub const USERNAME_MAX: usize = 32;
pub const PASSWORD_MIN: usize = 10;
/// Upper bound so a huge "password" cannot be used to burn CPU.
pub const PASSWORD_MAX: usize = 128;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// A session lasts this long after it was last used.
pub const SESSION_TTL_MS: i64 = 30 * DAY_MS;
/// `last_seen` is refreshed at most this often, so reads do not write on every request.
const TOUCH_EVERY_MS: i64 = 60 * 60 * 1000;

/// Failures allowed before backoff starts: strict per account, looser per address (people share networks).
const FREE_FAILURES_ACCOUNT: u32 = 5;
const FREE_FAILURES_ADDRESS: u32 = 20;
const BASE_LOCK_MS: i64 = 30_000;
const MAX_LOCK_MS: i64 = 15 * 60_000;
/// Failures older than this are forgotten.
const FAILURE_WINDOW_MS: i64 = 60 * 60_000;

#[derive(Debug, PartialEq, Eq)]
pub enum AccountError {
    InvalidUsername,
    /// Too short, too long, or the same as the username.
    InvalidPassword,
    UsernameTaken,
    /// Wrong username or password, or a disabled account. Deliberately one answer for all three.
    InvalidCredentials,
    RateLimited {
        retry_after_ms: i64,
    },
    RegistrationClosed,
    /// First-run setup was attempted but accounts already exist.
    SetupAlreadyDone,
    /// The signed-in user is not an admin.
    Forbidden,
    NotFound,
    /// The action would leave no enabled admin.
    LastAdmin,
    /// An admin tried to disable or delete their own account here.
    SelfAction,
    Db(String),
}

impl fmt::Display for AccountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AccountError {}

impl From<DbError> for AccountError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::UsernameTaken => AccountError::UsernameTaken,
            other => AccountError::Db(other.to_string()),
        }
    }
}

type Result<T> = std::result::Result<T, AccountError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Registration {
    Open,
    Closed,
}

/// A signed-in session: the cookie value to hand to the browser, and who it belongs to.
#[derive(Debug, Clone)]
pub struct Session {
    pub token: String,
    pub user: User,
}

pub struct Hasher {
    argon: Argon2<'static>,
}

impl Hasher {
    /// OWASP-recommended argon2id minimum: 19 MiB, 2 passes, 1 lane.
    pub fn new() -> Self {
        Self::with_params(19 * 1024, 2, 1)
    }

    /// Tiny cost so tests run fast. Never use outside tests.
    #[cfg(test)]
    pub fn cheap() -> Self {
        Self::with_params(8, 1, 1)
    }

    fn with_params(m_cost: u32, t_cost: u32, p_cost: u32) -> Self {
        // The constants above are valid, so this cannot fail; fall back to the library default if it ever does.
        let params = Params::new(m_cost, t_cost, p_cost, None).unwrap_or_default();
        Self {
            argon: Argon2::new(Algorithm::Argon2id, Version::V0x13, params),
        }
    }

    fn hash(&self, password: &str) -> Result<String> {
        let hash: PasswordHash = self
            .argon
            .hash_password(password.as_bytes())
            .map_err(|e| AccountError::Db(format!("hashing failed: {e}")))?;
        Ok(hash.to_string())
    }

    fn verify(&self, hash: &str, password: &str) -> bool {
        PasswordVerifier::<str>::verify_password(&self.argon, password.as_bytes(), hash).is_ok()
    }
}

impl Default for Hasher {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Accounts {
    hasher: Hasher,
    /// Verified against when the username does not exist, so a missing account costs the same time as a wrong password.
    dummy_hash: String,
}

fn sha256_hex(input: &str) -> String {
    Sha256::digest(input.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The prefix every personal API token starts with, so a leaked one is easy to recognise and search for.
pub const API_TOKEN_PREFIX: &str = "opt_";
/// Tokens a person may have at once.
pub const MAX_API_TOKENS: usize = 20;
/// A token's last-used time is only rewritten this often.
const TOKEN_TOUCH_EVERY_MS: i64 = 60_000;

fn new_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn validate_username(raw: &str) -> Result<String> {
    let name = raw.trim();
    let len = name.chars().count();
    let first_ok = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric());
    let all_ok = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if (USERNAME_MIN..=USERNAME_MAX).contains(&len) && first_ok && all_ok {
        Ok(name.to_string())
    } else {
        Err(AccountError::InvalidUsername)
    }
}

fn validate_password(password: &str, username: &str) -> Result<()> {
    let len = password.chars().count();
    if !(PASSWORD_MIN..=PASSWORD_MAX).contains(&len) || password.eq_ignore_ascii_case(username) {
        return Err(AccountError::InvalidPassword);
    }
    Ok(())
}

/// How long to lock after `failures` failures, given how many are free.
fn lock_ms(failures: u32, free: u32) -> i64 {
    if failures < free {
        return 0;
    }
    let doublings = (failures - free).min(16);
    (BASE_LOCK_MS << doublings).min(MAX_LOCK_MS)
}

impl Accounts {
    pub fn new(hasher: Hasher) -> Self {
        let dummy_hash = hasher.hash("not-a-real-password").unwrap_or_default();
        Self { hasher, dummy_hash }
    }

    // ---- setup and registration ----

    /// True until the first account exists.
    pub fn needs_setup(&self, db: &Db) -> Result<bool> {
        Ok(db.count_users()? == 0)
    }

    /// Create the first account, which becomes the admin, and sign it in. Only works on a fresh install.
    pub fn setup_admin(
        &self,
        db: &Db,
        username: &str,
        password: &str,
        now_ms: i64,
    ) -> Result<Session> {
        let username = validate_username(username)?;
        validate_password(password, &username)?;
        let user = self.new_user(&username, password, Role::Admin, now_ms)?;
        if !db.insert_user_if_none(&user)? {
            return Err(AccountError::SetupAlreadyDone);
        }
        self.start_session(db, user, now_ms)
    }

    pub fn registration(&self, db: &Db) -> Result<Registration> {
        Ok(match db.meta("registration")?.as_deref() {
            Some("open") => Registration::Open,
            _ => Registration::Closed,
        })
    }

    pub fn set_registration(&self, db: &Db, admin: &User, policy: Registration) -> Result<()> {
        require_admin(admin)?;
        let value = if policy == Registration::Open {
            "open"
        } else {
            "closed"
        };
        Ok(db.set_meta("registration", value)?)
    }

    /// Self-service sign-up, when the admin has opened registration. Signs the new user in.
    pub fn register(
        &self,
        db: &Db,
        username: &str,
        password: &str,
        now_ms: i64,
    ) -> Result<Session> {
        if self.needs_setup(db)? {
            return Err(AccountError::Forbidden);
        }
        if self.registration(db)? != Registration::Open {
            return Err(AccountError::RegistrationClosed);
        }
        let username = validate_username(username)?;
        validate_password(password, &username)?;
        let user = self.new_user(&username, password, Role::User, now_ms)?;
        db.insert_user(&user)?;
        self.start_session(db, user, now_ms)
    }

    /// An admin creates an account (works while registration is closed).
    pub fn admin_create_user(
        &self,
        db: &Db,
        admin: &User,
        username: &str,
        password: &str,
        role: Role,
        now_ms: i64,
    ) -> Result<User> {
        require_admin(admin)?;
        let username = validate_username(username)?;
        validate_password(password, &username)?;
        let user = self.new_user(&username, password, role, now_ms)?;
        db.insert_user(&user)?;
        Ok(user)
    }

    fn new_user(&self, username: &str, password: &str, role: Role, now_ms: i64) -> Result<User> {
        Ok(User {
            id: uuid::Uuid::new_v4().to_string(),
            username: username.to_string(),
            password_hash: self.hasher.hash(password)?,
            role,
            disabled: false,
            created_at: now_ms,
        })
    }

    // ---- sign in and sessions ----

    /// Check the password and start a session. `address` is the client address (as the server sees it) and is
    /// only used for rate limiting.
    pub fn sign_in(
        &self,
        db: &Db,
        username: &str,
        password: &str,
        address: &str,
        now_ms: i64,
    ) -> Result<Session> {
        let account_key = format!("user:{}", username.trim().to_ascii_lowercase());
        let address_key = format!("ip:{address}");

        let wait = [&account_key, &address_key]
            .into_iter()
            .map(|k| Ok(db.attempt(k)?.map_or(0, |a| a.locked_until - now_ms)))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .max()
            .unwrap_or(0);
        if wait > 0 {
            return Err(AccountError::RateLimited {
                retry_after_ms: wait,
            });
        }

        let user = db.user_by_name(username.trim())?;
        // Always verify something, so an unknown name and a wrong password take the same time.
        let hash = user
            .as_ref()
            .map_or(self.dummy_hash.as_str(), |u| u.password_hash.as_str());
        let matches = self.hasher.verify(hash, password);

        match user {
            Some(user) if matches && !user.disabled => {
                db.delete_attempt(&account_key)?;
                self.start_session(db, user, now_ms)
            }
            _ => {
                self.record_failure(db, &account_key, FREE_FAILURES_ACCOUNT, now_ms)?;
                self.record_failure(db, &address_key, FREE_FAILURES_ADDRESS, now_ms)?;
                db.purge_attempts_before(now_ms - 24 * 60 * 60_000)?;
                Err(AccountError::InvalidCredentials)
            }
        }
    }

    fn record_failure(&self, db: &Db, key: &str, free: u32, now_ms: i64) -> Result<()> {
        let previous = db
            .attempt(key)?
            .filter(|a| now_ms - a.updated_at < FAILURE_WINDOW_MS || a.locked_until > now_ms);
        let failures = previous.map_or(0, |a| a.failures).saturating_add(1);
        let locked_until = now_ms + lock_ms(failures, free);
        Ok(db.put_attempt(
            key,
            Attempt {
                failures,
                locked_until,
                updated_at: now_ms,
            },
        )?)
    }

    fn start_session(&self, db: &Db, user: User, now_ms: i64) -> Result<Session> {
        db.purge_expired_auth_sessions(now_ms)?;
        let token = new_token();
        db.insert_auth_session(
            &sha256_hex(&token),
            &user.id,
            now_ms,
            now_ms + SESSION_TTL_MS,
        )?;
        Ok(Session { token, user })
    }

    /// Who a session cookie belongs to, if it is valid, unexpired and the account is enabled.
    pub fn user_for_session(&self, db: &Db, token: &str, now_ms: i64) -> Result<Option<User>> {
        let hash = sha256_hex(token);
        let Some(session) = db.auth_session(&hash)? else {
            return Ok(None);
        };
        if session.expires_at <= now_ms {
            db.delete_auth_session(&hash)?;
            return Ok(None);
        }
        let Some(user) = db.user_by_id(&session.user_id)?.filter(|u| !u.disabled) else {
            return Ok(None);
        };
        if now_ms - session.last_seen >= TOUCH_EVERY_MS {
            db.touch_auth_session(&hash, now_ms, now_ms + SESSION_TTL_MS)?;
        }
        Ok(Some(user))
    }

    /// The id a session has in the user's session list (a short prefix of its stored hash).
    pub fn session_id(token: &str) -> String {
        sha256_hex(token).chars().take(16).collect()
    }

    /// Remember which browser a sign-in came from, for the session list.
    pub fn label_session(&self, db: &Db, token: &str, agent: &str) -> Result<()> {
        let agent: String = agent
            .chars()
            .filter(|c| !c.is_control())
            .take(200)
            .collect();
        Ok(db.set_session_agent(&sha256_hex(token), &agent)?)
    }

    // ---- personal API tokens ----

    /// Make a token for scripts. The secret is returned once and only its hash is stored.
    pub fn create_api_token(
        &self,
        db: &Db,
        user: &User,
        name: &str,
        now_ms: i64,
    ) -> Result<(String, String)> {
        let id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();
        let secret = format!("{API_TOKEN_PREFIX}{}", new_token());
        let name: String = name
            .trim()
            .chars()
            .filter(|c| !c.is_control())
            .take(60)
            .collect();
        db.insert_api_token(&id, &user.id, &name, &sha256_hex(&secret), now_ms)?;
        Ok((id, secret))
    }

    /// Who a personal API token belongs to, if it exists and the account is enabled.
    pub fn user_for_api_token(&self, db: &Db, token: &str, now_ms: i64) -> Result<Option<User>> {
        if !token.starts_with(API_TOKEN_PREFIX) {
            return Ok(None);
        }
        let Some((id, user_id, last_used)) = db.api_token_owner(&sha256_hex(token))? else {
            return Ok(None);
        };
        let Some(user) = db.user_by_id(&user_id)?.filter(|u| !u.disabled) else {
            return Ok(None);
        };
        if last_used.is_none_or(|t| now_ms - t >= TOKEN_TOUCH_EVERY_MS) {
            db.touch_api_token(&id, now_ms)?;
        }
        Ok(Some(user))
    }

    pub fn sign_out(&self, db: &Db, token: &str) -> Result<()> {
        Ok(db.delete_auth_session(&sha256_hex(token))?)
    }

    // ---- managing your own account ----

    /// Change your password. Every other session is signed out; the one passed in stays.
    pub fn change_password(
        &self,
        db: &Db,
        user: &User,
        current: &str,
        new: &str,
        keep_token: &str,
    ) -> Result<()> {
        if !self.hasher.verify(&user.password_hash, current) {
            return Err(AccountError::InvalidCredentials);
        }
        validate_password(new, &user.username)?;
        db.set_password_hash(&user.id, &self.hasher.hash(new)?)?;
        db.delete_auth_sessions_of(&user.id, Some(&sha256_hex(keep_token)))?;
        Ok(())
    }

    /// Delete your account and everything in it. The last admin cannot leave while other accounts exist.
    pub fn delete_account(&self, db: &Db, user: &User, password: &str) -> Result<()> {
        if !self.hasher.verify(&user.password_hash, password) {
            return Err(AccountError::InvalidCredentials);
        }
        if user.role == Role::Admin && db.count_enabled_admins()? <= 1 && db.count_users()? > 1 {
            return Err(AccountError::LastAdmin);
        }
        db.delete_user(&user.id)?;
        Ok(())
    }

    /// Delete everything the user has recorded, but keep the account. Needs the password.
    pub fn wipe_data(&self, db: &Db, user: &User, password: &str) -> Result<()> {
        if !self.hasher.verify(&user.password_hash, password) {
            return Err(AccountError::InvalidCredentials);
        }
        Ok(db.wipe_user_data(&user.id)?)
    }

    // ---- admin ----

    pub fn list_users(&self, db: &Db, admin: &User) -> Result<Vec<User>> {
        require_admin(admin)?;
        Ok(db.list_users()?)
    }

    /// Set a temporary password for someone else and sign them out everywhere.
    pub fn admin_set_password(
        &self,
        db: &Db,
        admin: &User,
        target_id: &str,
        new: &str,
    ) -> Result<()> {
        require_admin(admin)?;
        let target = db.user_by_id(target_id)?.ok_or(AccountError::NotFound)?;
        validate_password(new, &target.username)?;
        db.set_password_hash(&target.id, &self.hasher.hash(new)?)?;
        db.delete_auth_sessions_of(&target.id, None)?;
        Ok(())
    }

    pub fn admin_set_disabled(
        &self,
        db: &Db,
        admin: &User,
        target_id: &str,
        disabled: bool,
    ) -> Result<()> {
        require_admin(admin)?;
        if admin.id == target_id {
            return Err(AccountError::SelfAction);
        }
        let target = db.user_by_id(target_id)?.ok_or(AccountError::NotFound)?;
        if disabled
            && target.role == Role::Admin
            && !target.disabled
            && db.count_enabled_admins()? <= 1
        {
            return Err(AccountError::LastAdmin);
        }
        db.set_disabled(&target.id, disabled)?;
        if disabled {
            db.delete_auth_sessions_of(&target.id, None)?;
        }
        Ok(())
    }
}

fn require_admin(user: &User) -> Result<()> {
    if user.role == Role::Admin && !user.disabled {
        Ok(())
    } else {
        Err(AccountError::Forbidden)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PW: &str = "correct horse battery";
    const T0: i64 = 1_000_000;

    fn setup() -> (Accounts, Db, User) {
        let accounts = Accounts::new(Hasher::cheap());
        let db = Db::open_in_memory().expect("db");
        let admin = accounts
            .setup_admin(&db, "admin", PW, T0)
            .expect("setup")
            .user;
        (accounts, db, admin)
    }

    fn open_registration(a: &Accounts, db: &Db, admin: &User) {
        a.set_registration(db, admin, Registration::Open)
            .expect("open");
    }

    #[test]
    fn first_account_is_the_admin_and_setup_only_works_once() {
        let accounts = Accounts::new(Hasher::cheap());
        let db = Db::open_in_memory().expect("db");
        assert!(accounts.needs_setup(&db).expect("needs"));
        let session = accounts.setup_admin(&db, "admin", PW, T0).expect("setup");
        let admin = session.user.clone();
        assert_eq!(admin.role, Role::Admin);
        assert_eq!(
            accounts
                .user_for_session(&db, &session.token, T0)
                .expect("lookup")
                .map(|u| u.id),
            Some(admin.id)
        );
        assert!(!accounts.needs_setup(&db).expect("needs"));
        assert_eq!(
            accounts.setup_admin(&db, "second", PW, T0).err(),
            Some(AccountError::SetupAlreadyDone)
        );
        assert_eq!(db.count_users().expect("count"), 1);
    }

    #[test]
    fn passwords_are_stored_hashed() {
        let (_, db, admin) = setup();
        let stored = db.user_by_id(&admin.id).expect("get").expect("exists");
        assert!(stored.password_hash.starts_with("$argon2id$"));
        assert!(!stored.password_hash.contains(PW));
    }

    #[test]
    fn username_and_password_rules() {
        let accounts = Accounts::new(Hasher::cheap());
        let db = Db::open_in_memory().expect("db");
        for bad in [
            "",
            "ab",
            "has space",
            "-leading",
            "name!",
            &"x".repeat(33),
            "ünïcode",
        ] {
            assert_eq!(
                accounts.setup_admin(&db, bad, PW, T0).err(),
                Some(AccountError::InvalidUsername),
                "{bad:?}"
            );
        }
        for bad in ["short", &"p".repeat(129), "alice-alice"] {
            let name = if bad == "alice-alice" {
                "alice-alice"
            } else {
                "alice"
            };
            assert_eq!(
                accounts.setup_admin(&db, name, bad, T0).err(),
                Some(AccountError::InvalidPassword),
                "{bad:?}"
            );
        }
        assert!(
            accounts.setup_admin(&db, "  alice.b_c-1  ", PW, T0).is_ok(),
            "trimmed and valid"
        );
    }

    #[test]
    fn registration_is_closed_by_default_and_admin_can_open_it() {
        let (accounts, db, admin) = setup();
        assert_eq!(
            accounts.register(&db, "bob", PW, T0).err(),
            Some(AccountError::RegistrationClosed)
        );
        open_registration(&accounts, &db, &admin);
        let session = accounts.register(&db, "bob", PW, T0).expect("register");
        assert_eq!(session.user.role, Role::User);
        assert_eq!(
            accounts.register(&db, "BOB", PW, T0).err(),
            Some(AccountError::UsernameTaken)
        );
        accounts
            .set_registration(&db, &admin, Registration::Closed)
            .expect("close");
        assert_eq!(
            accounts.register(&db, "carol", PW, T0).err(),
            Some(AccountError::RegistrationClosed)
        );
    }

    #[test]
    fn only_admins_manage_accounts() {
        let (accounts, db, admin) = setup();
        let bob = accounts
            .admin_create_user(&db, &admin, "bob", PW, Role::User, T0)
            .expect("create");
        let err = accounts
            .admin_create_user(&db, &bob, "eve", PW, Role::User, T0)
            .err();
        assert_eq!(err, Some(AccountError::Forbidden));
        assert_eq!(
            accounts.list_users(&db, &bob).err(),
            Some(AccountError::Forbidden)
        );
        assert_eq!(
            accounts.set_registration(&db, &bob, Registration::Open),
            Err(AccountError::Forbidden)
        );
        assert_eq!(
            accounts.admin_set_password(&db, &bob, &admin.id, PW),
            Err(AccountError::Forbidden)
        );
        assert_eq!(
            accounts.admin_set_disabled(&db, &bob, &admin.id, true),
            Err(AccountError::Forbidden)
        );
        assert_eq!(accounts.list_users(&db, &admin).expect("list").len(), 2);
    }

    #[test]
    fn sign_in_gives_a_session_that_resolves_to_the_user() {
        let (accounts, db, admin) = setup();
        let session = accounts
            .sign_in(&db, " ADMIN ", PW, "1.2.3.4", T0)
            .expect("sign in");
        assert_eq!(session.user.id, admin.id);
        let found = accounts
            .user_for_session(&db, &session.token, T0 + 1)
            .expect("lookup");
        assert_eq!(found.map(|u| u.id), Some(admin.id));
        assert_eq!(
            accounts
                .user_for_session(&db, "garbage", T0)
                .expect("lookup")
                .map(|u| u.id),
            None
        );
    }

    #[test]
    fn only_a_hash_of_the_session_token_is_stored() {
        let (accounts, db, _) = setup();
        let session = accounts
            .sign_in(&db, "admin", PW, "a", T0)
            .expect("sign in");
        assert!(
            db.auth_session(&session.token)
                .expect("raw lookup")
                .is_none(),
            "raw token must not be a key"
        );
        assert!(
            db.auth_session(&sha256_hex(&session.token))
                .expect("hash lookup")
                .is_some()
        );
        assert!(session.token.len() >= 64);
    }

    #[test]
    fn wrong_password_unknown_user_and_disabled_account_look_the_same() {
        let (accounts, db, admin) = setup();
        let bob = accounts
            .admin_create_user(&db, &admin, "bob", PW, Role::User, T0)
            .expect("create");
        accounts
            .admin_set_disabled(&db, &admin, &bob.id, true)
            .expect("disable");
        let wrong = accounts
            .sign_in(&db, "admin", "wrong password!", "a", T0)
            .err();
        let unknown = accounts.sign_in(&db, "nobody", PW, "b", T0).err();
        let disabled = accounts.sign_in(&db, "bob", PW, "c", T0).err();
        assert_eq!(wrong, Some(AccountError::InvalidCredentials));
        assert_eq!(unknown, wrong);
        assert_eq!(disabled, wrong);
    }

    #[test]
    fn sessions_expire_and_slide_with_use() {
        let (accounts, db, _) = setup();
        let session = accounts
            .sign_in(&db, "admin", PW, "a", T0)
            .expect("sign in");
        let later = T0 + SESSION_TTL_MS - 1;
        assert!(
            accounts
                .user_for_session(&db, &session.token, later)
                .expect("lookup")
                .is_some()
        );
        // That use slid the expiry forward, so it is still valid a while after the original expiry.
        assert!(
            accounts
                .user_for_session(&db, &session.token, T0 + SESSION_TTL_MS + 1_000)
                .expect("lookup")
                .is_some()
        );
        let far = T0 + 3 * SESSION_TTL_MS;
        assert!(
            accounts
                .user_for_session(&db, &session.token, far)
                .expect("lookup")
                .is_none()
        );
    }

    #[test]
    fn sign_out_ends_only_that_session() {
        let (accounts, db, _) = setup();
        let a = accounts.sign_in(&db, "admin", PW, "x", T0).expect("a");
        let b = accounts.sign_in(&db, "admin", PW, "x", T0).expect("b");
        accounts.sign_out(&db, &a.token).expect("out");
        assert!(
            accounts
                .user_for_session(&db, &a.token, T0)
                .expect("a")
                .is_none()
        );
        assert!(
            accounts
                .user_for_session(&db, &b.token, T0)
                .expect("b")
                .is_some()
        );
    }

    #[test]
    fn rate_limit_locks_an_account_with_growing_backoff_then_recovers() {
        let (accounts, db, _) = setup();
        let mut now = T0;
        for _ in 0..FREE_FAILURES_ACCOUNT {
            assert_eq!(
                accounts
                    .sign_in(&db, "admin", "nope nope nope", "a", now)
                    .err(),
                Some(AccountError::InvalidCredentials)
            );
            now += 1_000;
        }
        // Even the right password is refused while locked, so guessing gains nothing.
        let locked = accounts.sign_in(&db, "admin", PW, "a", now).err();
        assert!(
            matches!(locked, Some(AccountError::RateLimited { retry_after_ms }) if retry_after_ms > 0)
        );
        // After the lock passes, signing in works and clears the counter.
        now += BASE_LOCK_MS + 1;
        assert!(accounts.sign_in(&db, "admin", PW, "a", now).is_ok());
        assert_eq!(db.attempt("user:admin").expect("attempt"), None);
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        assert_eq!(lock_ms(4, 5), 0);
        assert_eq!(lock_ms(5, 5), BASE_LOCK_MS);
        assert_eq!(lock_ms(6, 5), 2 * BASE_LOCK_MS);
        assert_eq!(lock_ms(7, 5), 4 * BASE_LOCK_MS);
        assert_eq!(lock_ms(500, 5), MAX_LOCK_MS);
    }

    #[test]
    fn one_address_guessing_many_accounts_is_locked_out() {
        let (accounts, db, _) = setup();
        let mut now = T0;
        for i in 0..FREE_FAILURES_ADDRESS {
            let name = format!("ghost{i}");
            assert_eq!(
                accounts
                    .sign_in(&db, &name, "nope nope nope", "9.9.9.9", now)
                    .err(),
                Some(AccountError::InvalidCredentials)
            );
            now += 100;
        }
        let blocked = accounts.sign_in(&db, "admin", PW, "9.9.9.9", now).err();
        assert!(matches!(blocked, Some(AccountError::RateLimited { .. })));
        // Another address is unaffected.
        assert!(accounts.sign_in(&db, "admin", PW, "8.8.8.8", now).is_ok());
    }

    #[test]
    fn old_failures_are_forgotten() {
        let (accounts, db, _) = setup();
        for i in 0..(FREE_FAILURES_ACCOUNT - 1) {
            let _ = accounts.sign_in(&db, "admin", "nope nope nope", "a", T0 + i64::from(i));
        }
        let much_later = T0 + FAILURE_WINDOW_MS + 100;
        let _ = accounts.sign_in(&db, "admin", "nope nope nope", "a", much_later);
        let attempt = db.attempt("user:admin").expect("attempt").expect("exists");
        assert_eq!(attempt.failures, 1, "the window restarted");
    }

    #[test]
    fn changing_password_checks_the_current_one_and_signs_out_other_sessions() {
        let (accounts, db, admin) = setup();
        let here = accounts.sign_in(&db, "admin", PW, "x", T0).expect("here");
        let elsewhere = accounts
            .sign_in(&db, "admin", PW, "x", T0)
            .expect("elsewhere");
        let wrong = accounts.change_password(
            &db,
            &admin,
            "not it at all",
            "a brand new password",
            &here.token,
        );
        assert_eq!(wrong, Err(AccountError::InvalidCredentials));
        let weak = accounts.change_password(&db, &admin, PW, "short", &here.token);
        assert_eq!(weak, Err(AccountError::InvalidPassword));
        accounts
            .change_password(&db, &admin, PW, "a brand new password", &here.token)
            .expect("change");
        assert!(
            accounts
                .user_for_session(&db, &here.token, T0)
                .expect("here")
                .is_some()
        );
        assert!(
            accounts
                .user_for_session(&db, &elsewhere.token, T0)
                .expect("elsewhere")
                .is_none()
        );
        assert!(
            accounts.sign_in(&db, "admin", PW, "x", T0).is_err(),
            "old password no longer works"
        );
        assert!(
            accounts
                .sign_in(&db, "admin", "a brand new password", "x", T0)
                .is_ok()
        );
    }

    #[test]
    fn admin_temporary_password_signs_the_user_out() {
        let (accounts, db, admin) = setup();
        let bob = accounts
            .admin_create_user(&db, &admin, "bob", PW, Role::User, T0)
            .expect("create");
        let session = accounts.sign_in(&db, "bob", PW, "x", T0).expect("sign in");
        accounts
            .admin_set_password(&db, &admin, &bob.id, "temporary password 1")
            .expect("set");
        assert!(
            accounts
                .user_for_session(&db, &session.token, T0)
                .expect("lookup")
                .is_none()
        );
        assert!(
            accounts
                .sign_in(&db, "bob", "temporary password 1", "x", T0)
                .is_ok()
        );
        assert_eq!(
            accounts.admin_set_password(&db, &admin, "missing", PW),
            Err(AccountError::NotFound)
        );
    }

    #[test]
    fn disabling_signs_the_user_out_and_blocks_sign_in_until_enabled() {
        let (accounts, db, admin) = setup();
        let bob = accounts
            .admin_create_user(&db, &admin, "bob", PW, Role::User, T0)
            .expect("create");
        let session = accounts.sign_in(&db, "bob", PW, "x", T0).expect("sign in");
        accounts
            .admin_set_disabled(&db, &admin, &bob.id, true)
            .expect("disable");
        assert!(
            accounts
                .user_for_session(&db, &session.token, T0)
                .expect("lookup")
                .is_none()
        );
        assert!(accounts.sign_in(&db, "bob", PW, "y", T0).is_err());
        accounts
            .admin_set_disabled(&db, &admin, &bob.id, false)
            .expect("enable");
        assert!(accounts.sign_in(&db, "bob", PW, "z", T0).is_ok());
    }

    #[test]
    fn an_admin_cannot_disable_themselves_but_can_disable_another_admin() {
        let (accounts, db, admin) = setup();
        assert_eq!(
            accounts.admin_set_disabled(&db, &admin, &admin.id, true),
            Err(AccountError::SelfAction)
        );
        let second = accounts
            .admin_create_user(&db, &admin, "second", PW, Role::Admin, T0)
            .expect("create");
        accounts
            .admin_set_disabled(&db, &admin, &second.id, true)
            .expect("disable the other admin");
        assert_eq!(
            db.count_enabled_admins().expect("count"),
            1,
            "the acting admin always stays enabled"
        );
        // A disabled admin has no admin rights left.
        let disabled = db.user_by_id(&second.id).expect("get").expect("exists");
        assert_eq!(
            accounts.list_users(&db, &disabled).err(),
            Some(AccountError::Forbidden)
        );
    }

    #[test]
    fn deleting_an_account_needs_the_password_and_wipes_its_data() {
        let (accounts, db, admin) = setup();
        let bob = accounts
            .admin_create_user(&db, &admin, "bob", PW, Role::User, T0)
            .expect("create");
        let session = accounts.sign_in(&db, "bob", PW, "x", T0).expect("sign in");
        assert_eq!(
            accounts.delete_account(&db, &bob, "wrong wrong wrong"),
            Err(AccountError::InvalidCredentials)
        );
        accounts.delete_account(&db, &bob, PW).expect("delete");
        assert_eq!(db.user_by_id(&bob.id).expect("get"), None);
        assert!(
            accounts
                .user_for_session(&db, &session.token, T0)
                .expect("lookup")
                .is_none()
        );
    }

    #[test]
    fn the_last_admin_cannot_delete_their_account_while_others_remain() {
        let (accounts, db, admin) = setup();
        accounts
            .admin_create_user(&db, &admin, "bob", PW, Role::User, T0)
            .expect("create");
        assert_eq!(
            accounts.delete_account(&db, &admin, PW),
            Err(AccountError::LastAdmin)
        );
    }

    #[test]
    fn the_only_account_can_be_deleted_and_setup_runs_again() {
        let (accounts, db, admin) = setup();
        accounts.delete_account(&db, &admin, PW).expect("delete");
        assert!(accounts.needs_setup(&db).expect("needs"));
    }
}
