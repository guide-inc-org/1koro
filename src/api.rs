use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::agent::Agent;

#[derive(Clone)]
pub struct AppState {
    pub agent: Arc<Agent>,
    pub name: String,
    pub api_key: Option<String>,
}

#[derive(Deserialize)]
pub struct MessageRequest {
    pub text: String,
    #[serde(default = "default_channel")]
    pub channel: String,
    #[serde(default = "default_user")]
    pub user: String,
}

fn default_channel() -> String {
    "cli".into()
}
fn default_user() -> String {
    "masaki".into()
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub text: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<serde_json::Value>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(chat_page))
        .route("/message", post(handle_message))
        .route("/health", get(handle_health))
        .layer(middleware::from_fn_with_state(state.clone(), auth_layer))
        .with_state(state)
}

async fn auth_layer(State(state): State<AppState>, req: Request, next: Next) -> impl IntoResponse {
    if let Some(ref expected) = state.api_key
        && !matches!(req.uri().path(), "/health" | "/")
    {
        let auth_ok = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .is_some_and(|t| t == expected);
        if !auth_ok {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Unauthorized"})),
            )
                .into_response();
        }
    }
    next.run(req).await.into_response()
}

async fn handle_message(
    State(state): State<AppState>,
    Json(req): Json<MessageRequest>,
) -> impl IntoResponse {
    match state
        .agent
        .handle_message(&req.text, &req.channel, &req.user)
        .await
    {
        Ok(resp) => (
            StatusCode::OK,
            Json(MessageResponse {
                text: resp.text.unwrap_or_else(|| "(no response)".into()),
                actions: resp.actions,
            }),
        ),
        Err(e) => {
            tracing::error!("Agent error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MessageResponse {
                    text: format!("Error: {e}"),
                    actions: vec![],
                }),
            )
        }
    }
}

async fn handle_health(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "name": state.name,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn chat_page(State(state): State<AppState>) -> impl IntoResponse {
    let token = state.api_key.as_deref().unwrap_or("");
    let html = CHAT_HTML.replace("{{API_TOKEN}}", token);
    axum::response::Html(html)
}

const CHAT_HTML: &str = r##"<!DOCTYPE html>
<html lang="ja">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>1koro</title>
<style>
*,*::before,*::after{box-sizing:border-box;margin:0;padding:0}
:root{
  --bg:#ffffff;--bg-chat:#f7f7f8;--bg-user:#2563eb;--bg-bot:#e5e7eb;
  --text:#1a1a1a;--text-user:#ffffff;--text-bot:#1a1a1a;--text-muted:#6b7280;
  --border:#e5e7eb;--input-bg:#ffffff;--input-border:#d1d5db;
  --header-bg:#ffffff;--header-border:#e5e7eb;
  --code-bg:#f3f4f6;--code-border:#e5e7eb;
  --pre-bg:#1e1e1e;--pre-text:#d4d4d4;
}
@media(prefers-color-scheme:dark){
  :root{
    --bg:#1a1a1a;--bg-chat:#0d0d0d;--bg-user:#2563eb;--bg-bot:#2d2d2d;
    --text:#e5e5e5;--text-user:#ffffff;--text-bot:#e5e5e5;--text-muted:#9ca3af;
    --border:#333;--input-bg:#2d2d2d;--input-border:#444;
    --header-bg:#1a1a1a;--header-border:#333;
    --code-bg:#333;--code-border:#444;
    --pre-bg:#0d0d0d;--pre-text:#d4d4d4;
  }
}
html,body{height:100%;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;background:var(--bg-chat);color:var(--text)}
#app{display:flex;flex-direction:column;height:100%;max-width:768px;margin:0 auto;background:var(--bg)}
header{padding:12px 20px;border-bottom:1px solid var(--header-border);background:var(--header-bg);display:flex;align-items:center;gap:10px;flex-shrink:0}
header .avatar{width:32px;height:32px;border-radius:50%;background:linear-gradient(135deg,#6366f1,#8b5cf6);display:flex;align-items:center;justify-content:center;color:#fff;font-weight:700;font-size:14px}
header h1{font-size:16px;font-weight:600}
#messages{flex:1;overflow-y:auto;padding:20px;display:flex;flex-direction:column;gap:16px}
.msg{display:flex;gap:10px;max-width:85%;animation:fadeIn .2s ease}
.msg.user{align-self:flex-end;flex-direction:row-reverse}
.msg .avatar{width:28px;height:28px;border-radius:50%;flex-shrink:0;display:flex;align-items:center;justify-content:center;font-size:12px;font-weight:700;margin-top:2px}
.msg.bot .avatar{background:linear-gradient(135deg,#6366f1,#8b5cf6);color:#fff}
.msg.user .avatar{background:var(--bg-user);color:#fff}
.bubble{padding:10px 14px;border-radius:18px;line-height:1.5;font-size:14px;white-space:pre-wrap;word-break:break-word}
.msg.user .bubble{background:var(--bg-user);color:var(--text-user);border-bottom-right-radius:4px}
.msg.bot .bubble{background:var(--bg-bot);color:var(--text-bot);border-bottom-left-radius:4px}
.bubble p{margin:0 0 8px}.bubble p:last-child{margin:0}
.bubble strong{font-weight:700}
.bubble em{font-style:italic}
.bubble code{background:var(--code-bg);padding:1px 5px;border-radius:4px;font-family:"SF Mono",Monaco,Consolas,monospace;font-size:13px}
.bubble pre{background:var(--pre-bg);color:var(--pre-text);padding:12px;border-radius:8px;overflow-x:auto;margin:8px 0;position:relative}
.bubble pre code{background:none;padding:0;color:inherit}
.bubble ul,.bubble ol{margin:4px 0 8px 20px}
.bubble li{margin:2px 0}
.bubble a{color:#60a5fa;text-decoration:underline}
.copy-btn{position:absolute;top:6px;right:6px;background:rgba(255,255,255,.15);border:none;color:#aaa;cursor:pointer;padding:3px 8px;border-radius:4px;font-size:11px}
.copy-btn:hover{background:rgba(255,255,255,.3);color:#fff}
.typing{display:flex;gap:10px;align-self:flex-start;max-width:85%}
.typing .dots{display:flex;gap:4px;padding:12px 16px;background:var(--bg-bot);border-radius:18px;border-bottom-left-radius:4px}
.typing .dots span{width:8px;height:8px;background:var(--text-muted);border-radius:50%;animation:bounce .6s infinite alternate}
.typing .dots span:nth-child(2){animation-delay:.2s}
.typing .dots span:nth-child(3){animation-delay:.4s}
@keyframes bounce{to{opacity:.3;transform:translateY(-4px)}}
@keyframes fadeIn{from{opacity:0;transform:translateY(8px)}to{opacity:1;transform:translateY(0)}}
#input-area{padding:12px 20px 20px;border-top:1px solid var(--border);background:var(--bg);flex-shrink:0}
#input-wrap{display:flex;gap:8px;align-items:flex-end;background:var(--input-bg);border:1px solid var(--input-border);border-radius:16px;padding:8px 12px}
#input{flex:1;border:none;outline:none;resize:none;background:transparent;color:var(--text);font-size:14px;font-family:inherit;line-height:1.5;max-height:120px;overflow-y:auto}
#input::placeholder{color:var(--text-muted)}
#send{width:32px;height:32px;border-radius:50%;border:none;background:var(--bg-user);color:#fff;cursor:pointer;display:flex;align-items:center;justify-content:center;flex-shrink:0;transition:opacity .15s}
#send:disabled{opacity:.4;cursor:default}
#send svg{width:16px;height:16px}
.empty-state{display:flex;flex-direction:column;align-items:center;justify-content:center;flex:1;color:var(--text-muted);gap:12px}
.empty-state .icon{width:48px;height:48px;border-radius:50%;background:linear-gradient(135deg,#6366f1,#8b5cf6);display:flex;align-items:center;justify-content:center;color:#fff;font-size:24px;font-weight:700}
.empty-state p{font-size:14px}
.error-toast{position:fixed;top:16px;left:50%;transform:translateX(-50%);background:#ef4444;color:#fff;padding:10px 20px;border-radius:8px;font-size:13px;z-index:100;animation:fadeIn .2s ease}
</style>
</head>
<body>
<div id="app">
  <header>
    <div class="avatar">1</div>
    <h1>1koro</h1>
  </header>
  <div id="messages">
    <div class="empty-state" id="empty">
      <div class="icon">1</div>
      <p>1koro にメッセージを送ってみよう</p>
    </div>
  </div>
  <div id="input-area">
    <div id="input-wrap">
      <textarea id="input" rows="1" placeholder="メッセージを入力..."></textarea>
      <button id="send" disabled><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/></svg></button>
    </div>
  </div>
</div>
<script>
(function(){
const TOKEN="{{API_TOKEN}}";
const messagesEl=document.getElementById("messages");
const inputEl=document.getElementById("input");
const sendBtn=document.getElementById("send");
const emptyEl=document.getElementById("empty");
let sending=false;

inputEl.addEventListener("input",()=>{
  inputEl.style.height="auto";
  inputEl.style.height=Math.min(inputEl.scrollHeight,120)+"px";
  sendBtn.disabled=!inputEl.value.trim()||sending;
});
inputEl.addEventListener("keydown",e=>{
  if(e.key==="Enter"&&!e.shiftKey){e.preventDefault();doSend()}
});
sendBtn.addEventListener("click",doSend);

function doSend(){
  const text=inputEl.value.trim();
  if(!text||sending)return;
  if(emptyEl)emptyEl.remove();
  addMsg("user",text);
  inputEl.value="";inputEl.style.height="auto";
  sendBtn.disabled=true;sending=true;
  const typing=showTyping();
  fetch("/message",{
    method:"POST",
    headers:{"Content-Type":"application/json",
      ...(TOKEN?{"Authorization":"Bearer "+TOKEN}:{})},
    body:JSON.stringify({text,channel:"web",user:"masaki"})
  }).then(r=>{
    if(!r.ok)throw new Error(r.status+" "+r.statusText);
    return r.json();
  }).then(d=>{
    typing.remove();
    addMsg("bot",d.text||"(no response)");
  }).catch(e=>{
    typing.remove();
    showError("Error: "+e.message);
  }).finally(()=>{
    sending=false;
    sendBtn.disabled=!inputEl.value.trim();
    inputEl.focus();
  });
}

function addMsg(role,text){
  const wrap=document.createElement("div");
  wrap.className="msg "+role;
  const av=document.createElement("div");
  av.className="avatar";
  av.textContent=role==="bot"?"1":"M";
  const bub=document.createElement("div");
  bub.className="bubble";
  bub.innerHTML=role==="bot"?renderMd(text):escHtml(text).replace(/\n/g,"<br>");
  wrap.appendChild(av);wrap.appendChild(bub);
  messagesEl.appendChild(wrap);
  messagesEl.scrollTop=messagesEl.scrollHeight;
}

function showTyping(){
  const el=document.createElement("div");
  el.className="typing";
  el.innerHTML='<div class="avatar" style="width:28px;height:28px;border-radius:50%;background:linear-gradient(135deg,#6366f1,#8b5cf6);color:#fff;display:flex;align-items:center;justify-content:center;font-size:12px;font-weight:700">1</div><div class="dots"><span></span><span></span><span></span></div>';
  messagesEl.appendChild(el);
  messagesEl.scrollTop=messagesEl.scrollHeight;
  return el;
}

function showError(msg){
  const el=document.createElement("div");
  el.className="error-toast";el.textContent=msg;
  document.body.appendChild(el);
  setTimeout(()=>el.remove(),4000);
}

function escHtml(s){return s.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;")}

function renderMd(src){
  // code blocks
  let html=escHtml(src);
  html=html.replace(/```(\w*)\n([\s\S]*?)```/g,(_,lang,code)=>{
    const id="cb"+Math.random().toString(36).slice(2,8);
    return '<pre><code class="lang-'+lang+'" id="'+id+'">'+code.replace(/\n$/,"")+'</code><button class="copy-btn" onclick="navigator.clipboard.writeText(document.getElementById(\''+id+'\').textContent)">copy</button></pre>';
  });
  // inline code
  html=html.replace(/`([^`]+)`/g,'<code>$1</code>');
  // bold
  html=html.replace(/\*\*(.+?)\*\*/g,'<strong>$1</strong>');
  // italic
  html=html.replace(/(?<!\*)\*([^*]+)\*(?!\*)/g,'<em>$1</em>');
  // links
  html=html.replace(/\[([^\]]+)\]\(([^)]+)\)/g,'<a href="$2" target="_blank" rel="noopener">$1</a>');
  // unordered list
  html=html.replace(/(^|\n)- (.+)/g,'$1<li>$2</li>');
  html=html.replace(/((?:<li>.*<\/li>\n?)+)/g,'<ul>$1</ul>');
  // paragraphs
  html=html.replace(/\n{2,}/g,'</p><p>');
  html=html.replace(/\n/g,'<br>');
  html='<p>'+html+'</p>';
  html=html.replace(/<p><\/p>/g,'');
  return html;
}

inputEl.focus();
})();
</script>
</body>
</html>"##;
