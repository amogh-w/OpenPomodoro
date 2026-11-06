/* theme.js: load synchronously in <head> so the theme is set before first paint. Local only, no network. */
var FLAVORS=["latte","frappe","macchiato","mocha","contrast-light","contrast-dark","nord","gruvbox","solarized"],ACCENTS=["rosewater","flamingo","pink","mauve","red","peach","yellow","green","teal","sky","blue","lavender"];
var LIGHT_FLAVORS=["latte","contrast-light","solarized"];
var CUSTOM_KEYS=["rosewater","flamingo","pink","mauve","red","maroon","peach","yellow","green","teal","sky","sapphire","blue","lavender","text","subtext1","subtext0","overlay2","overlay1","overlay0","surface2","surface1","surface0","base","mantle","crust"];
var RADII={tight:.55,apple:1,soft:1.5};
var store={get:function(k){try{return localStorage.getItem(k)}catch(e){return null}},set:function(k,v){try{localStorage.setItem(k,v)}catch(e){}}};
var mq=matchMedia("(prefers-color-scheme: dark)");
/* A person's own theme: {"base":"mocha","colors":{"base":"#112233",...}} in ui-custom. Anything that is not a known
   base flavor or a #rrggbb colour is ignored, so a bad file can never break the page. Returns null when unusable. */
function parseCustomTheme(text){
  var o; try{o=JSON.parse(text)}catch(e){return null}
  if(!o||typeof o!=="object"||Array.isArray(o))return null;
  var base=FLAVORS.indexOf(o.base)>=0?o.base:"mocha",colors={},c=o.colors&&typeof o.colors==="object"?o.colors:{},n=0;
  CUSTOM_KEYS.forEach(function(k){var v=c[k]; if(typeof v==="string"&&/^#[0-9a-fA-F]{6}$/.test(v)){colors[k]=v;n++}});
  return n?{base:base,colors:colors}:null;
}
function applyTheme(){
  var f=store.get("ui-flavor")||"auto",a=store.get("ui-accent")||"lavender",r=store.get("ui-radius")||"apple",h=document.documentElement;
  var custom=null;
  if(f==="custom"){custom=parseCustomTheme(store.get("ui-custom")||""); if(!custom)f="auto";}
  if(f!=="auto"&&f!=="custom"&&FLAVORS.indexOf(f)<0)f="auto"; if(ACCENTS.indexOf(a)<0)a="lavender"; if(!RADII[r])r="apple";
  var fl=custom?custom.base:f==="auto"?(mq.matches?"mocha":"latte"):f;
  FLAVORS.forEach(function(x){h.classList.toggle(x,x===fl)});
  h.classList.toggle("ui-light",LIGHT_FLAVORS.indexOf(fl)>=0);
  CUSTOM_KEYS.forEach(function(k){if(custom&&custom.colors[k])h.style.setProperty("--ctp-"+k,custom.colors[k]);else h.style.removeProperty("--ctp-"+k)});
  h.style.setProperty("--accent","var(--ctp-"+a+")"); h.style.setProperty("--r-scale",RADII[r]);
  document.querySelectorAll("[data-flavor]").forEach(function(b){b.setAttribute("aria-pressed",b.dataset.flavor===f)});
  document.querySelectorAll("[data-accent]").forEach(function(b){b.classList.toggle("ring-2",b.dataset.accent===a)});
  document.querySelectorAll("[data-radius]").forEach(function(b){b.setAttribute("aria-pressed",b.dataset.radius===r)});
  document.querySelectorAll(".seg").forEach(function(s){s.dispatchEvent(new Event("sync"))});
}
applyTheme(); mq.addEventListener&&mq.addEventListener("change",applyTheme);
/* Another tab changed the theme: follow it at once. */
addEventListener("storage",function(e){if(e.key&&e.key.indexOf("ui-")===0)applyTheme()});
