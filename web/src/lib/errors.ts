import { ApiError } from "./api";
import { type MessageKey, t } from "./i18n";

const messages: Record<string, MessageKey> = {
  invalid_credentials: "error.invalid_credentials",
  invalid_username: "error.invalid_username",
  invalid_password: "error.invalid_password",
  username_taken: "error.username_taken",
  registration_closed: "error.registration_closed",
  setup_done: "error.setup_done",
  last_admin: "error.last_admin",
  self_action: "error.self_action",
  forbidden: "error.forbidden",
  not_found: "error.not_found",
  bad_backup: "error.bad_backup",
  backup_too_large: "error.backup_too_large",
  payload_too_large: "error.payload_too_large",
  too_many_tokens: "error.too_many_tokens",
  invalid_code: "error.invalid_code",
  totp_state: "error.totp_state",
  name_required: "error.name_required",
};

/** A sentence a person can read for whatever went wrong. */
export function errorMessage(error: unknown): string {
  if (!(error instanceof ApiError)) return t("error.unknown");
  if (error.isNetwork) return t("error.network");
  if (error.code === "rate_limited") {
    return t("error.rate_limited", { seconds: Math.max(1, Math.ceil((error.retryAfterMs ?? 30_000) / 1000)) });
  }
  const key = messages[error.code];
  return key ? t(key) : t("error.unknown");
}
