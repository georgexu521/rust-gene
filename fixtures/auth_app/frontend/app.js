const API_URL = 'http://127.0.0.1:8080';
let authToken = localStorage.getItem('token') || '';

async function register() {
  const username = document.getElementById('reg-username').value.trim();
  const password = document.getElementById('reg-password').value;
  const errEl = document.getElementById('reg-error');
  errEl.textContent = '';
  try {
    const resp = await fetch(API_URL + '/register', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password })
    });
    const data = await resp.json();
    if (!resp.ok) {
      errEl.textContent = data.error || 'register failed';
      return;
    }
    showLogin();
    document.getElementById('login-username').value = username;
  } catch (e) {
    errEl.textContent = 'network error';
  }
}

async function login() {
  const username = document.getElementById('login-username').value.trim();
  const password = document.getElementById('login-password').value;
  const errEl = document.getElementById('login-error');
  errEl.textContent = '';
  try {
    const resp = await fetch(API_URL + '/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password })
    });
    const data = await resp.json();
    if (!resp.ok) {
      errEl.textContent = data.error || 'login failed';
      return;
    }
    authToken = data.token;
    localStorage.setItem('token', authToken);
    localStorage.setItem('username', data.username || username);
    document.getElementById('current-user').textContent = data.username || username;
    showApp();
    loadTodos();
  } catch (e) {
    errEl.textContent = 'network error';
  }
}

function logout() {
  authToken = '';
  localStorage.removeItem('token');
  localStorage.removeItem('username');
  showAuth();
}

async function addTodo(e) {
  e.preventDefault();
  const textEl = document.getElementById('todo-text');
  const text = textEl.value.trim();
  if (!text) return;
  try {
    const resp = await fetch(API_URL + '/todos', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': 'Bearer ' + authToken
      },
      body: JSON.stringify({ text })
    });
    if (resp.status === 401) { logout(); return; }
    if (!resp.ok) return;
    textEl.value = '';
    loadTodos();
  } catch (e) {
    // ignore network errors
  }
}

async function loadTodos() {
  const list = document.getElementById('todos');
  list.innerHTML = '';
  try {
    const resp = await fetch(API_URL + '/todos', {
      headers: { 'Authorization': 'Bearer ' + authToken }
    });
    if (resp.status === 401) { logout(); return; }
    const items = await resp.json();
    items.forEach(item => {
      const div = document.createElement('div');
      div.className = 'todo-item' + (item.done ? ' done' : '');
      const cb = document.createElement('input');
      cb.type = 'checkbox';
      cb.checked = !!item.done;
      cb.addEventListener('change', () => toggleTodo(item.id));
      const span = document.createElement('span');
      span.textContent = item.text;
      span.style.flex = '1';
      span.style.marginLeft = '10px';
      const del = document.createElement('button');
      del.textContent = 'Delete';
      del.addEventListener('click', () => deleteTodo(item.id));
      div.appendChild(cb);
      div.appendChild(span);
      div.appendChild(del);
      list.appendChild(div);
    });
  } catch (e) {
    // ignore
  }
}

async function deleteTodo(id) {
  try {
    const resp = await fetch(API_URL + '/todos/' + encodeURIComponent(id), {
      method: 'DELETE',
      headers: { 'Authorization': 'Bearer ' + authToken }
    });
    if (resp.status === 401) { logout(); return; }
    loadTodos();
  } catch (e) {
    // ignore
  }
}

async function toggleTodo(id) {
  // Backend does not support toggle yet; trigger a refresh after a no-op state flip locally.
  loadTodos();
}
function showRegister() { document.getElementById('login-form').classList.add('hidden'); document.getElementById('register-form').classList.remove('hidden'); }
function showLogin() { document.getElementById('register-form').classList.add('hidden'); document.getElementById('login-form').classList.remove('hidden'); }
function showApp() { document.getElementById('auth-section').classList.add('hidden'); document.getElementById('app-section').classList.remove('hidden'); }
function showAuth() { document.getElementById('app-section').classList.add('hidden'); document.getElementById('auth-section').classList.remove('hidden'); }

// Check if already logged in
if (authToken) showApp();
