let ws = null;
let term = null;

// --- Virtual Key Functions ---
function sendKey(name) {
    if (!term) return;
    term.focus();
    const seqMap = {
        'Tab':     '\t',
        'Esc':     '\x1b',
        'ArrowUp':    '\x1b[A',
        'ArrowDown':  '\x1b[B',
        'ArrowRight': '\x1b[C',
        'ArrowLeft':  '\x1b[D',
        'Home':    '\x1b[H',
        'End':     '\x1b[F',
        'Delete':  '\x1b[3~',
        'Insert':  '\x1b[2~',
        'PageUp':  '\x1b[5~',
        'PageDown':'\x1b[6~',
    };
    const seq = seqMap[name];
    if (seq && ws && ws.readyState === WebSocket.OPEN) {
        ws.send(new TextEncoder().encode(seq));
    }
}

function sendCtrl(ch) {
    if (!term) return;
    term.focus();
    // Ctrl+A = 0x01, Ctrl+C = 0x03, etc.
    const code = ch.charCodeAt(0) - 96;
    if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(new Uint8Array([code]));
    }
}

function sendSeq(suffix) {
    if (!term) return;
    term.focus();
    if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(new TextEncoder().encode('\x1b' + suffix));
    }
}

function toggleKeybar() {
    document.getElementById('keybar').classList.toggle('show');
}

// --- UI Helpers ---
function clearMsgs() {
    document.getElementById('error-msg').textContent = '';
    document.getElementById('success-msg').textContent = '';
}
function showError(msg) { document.getElementById('error-msg').textContent = msg; }
function showSuccess(msg) { document.getElementById('success-msg').textContent = msg; }

function showChangePassword() {
    clearMsgs();
    document.getElementById('auth-page').style.display = 'flex';
    document.getElementById('shell-page').style.display = 'none';
    document.getElementById('login-form').style.display = 'none';
    document.getElementById('change-pw-form').style.display = 'block';
    document.getElementById('form-subtitle').textContent = '修改密码';
    if (term) { term.dispose(); term = null; }
    if (ws) { ws.close(); ws = null; }
}

function hideChangePassword() {
    startShell();
}

async function apiFetch(url, body) {
    const res = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body)
    });
    const text = await res.text();
    let data;
    try { data = JSON.parse(text); } catch { data = { message: text }; }
    if (!res.ok) throw new Error(data.message || 'Request failed');
    return data;
}

async function doLogin() {
    clearMsgs();
    const username = document.getElementById('login-username').value.trim();
    const password = document.getElementById('login-password').value;
    if (!username || !password) return showError('请填写用户名和密码');

    try {
        const data = await apiFetch('./api/login', { username, password });
        localStorage.setItem('token', data.token);
        localStorage.setItem('username', data.username);
        startShell();
    } catch (e) { showError(e.message); }
}

async function doChangePassword() {
    clearMsgs();
    const username = localStorage.getItem('username');
    const old_password = document.getElementById('old-pw').value;
    const new_password = document.getElementById('new-pw').value;
    if (!old_password || !new_password) return showError('请填写所有字段');

    try {
        await apiFetch('./api/change-password', { username, old_password, new_password });
        showSuccess('密码修改成功！3秒后返回终端...');
        setTimeout(() => hideChangePassword(), 3000);
    } catch (e) { showError(e.message); }
}

function doLogout() {
    localStorage.removeItem('token');
    localStorage.removeItem('username');
    if (term) { term.dispose(); term = null; }
    if (ws) { ws.close(); ws = null; }
    document.getElementById('auth-page').style.display = 'flex';
    document.getElementById('shell-page').style.display = 'none';
    document.getElementById('login-form').style.display = 'block';
    document.getElementById('change-pw-form').style.display = 'none';
    document.getElementById('form-subtitle').textContent = '登录以访问终端';
}

async function doUpload(input) {
    const file = input.files[0];
    if (!file) return;
    input.value = '';

    const token = localStorage.getItem('token');
    if (!token) return doLogout();

    const sizeMB = (file.size / 1024 / 1024).toFixed(1);
    if (term) term.write('\r\n\x1b[36m[上传中] ' + file.name + ' (' + sizeMB + 'MB)...\x1b[0m');

    const formData = new FormData();
    formData.append('file', file);

    try {
        const res = await fetch('./api/upload', {
            method: 'POST',
            headers: { 'Authorization': 'Bearer ' + token },
            body: formData
        });

        const text = await res.text();
        let data;
        try {
            data = JSON.parse(text);
        } catch {
            if (res.status === 413) {
                throw new Error('文件太大，服务器拒绝接收（最大 100MB）');
            } else if (res.status === 401) {
                throw new Error('登录已过期，请重新登录');
            } else {
                throw new Error('服务器错误 (HTTP ' + res.status + ')');
            }
        }

        if (!res.ok) throw new Error(data.message || '上传失败');
        if (term) term.write('\r\n\x1b[32m[上传成功] ' + data.message + '\x1b[0m\r\n');
    } catch (e) {
        if (term) term.write('\r\n\x1b[31m[上传失败] ' + e.message + '\x1b[0m\r\n');
    }
}

function startShell() {
    const token = localStorage.getItem('token');
    const username = localStorage.getItem('username');
    if (!token) return doLogout();

    document.getElementById('auth-page').style.display = 'none';
    document.getElementById('shell-page').style.display = 'flex';
    document.getElementById('user-info').textContent = '用户: ' + username;

    if (term) term.dispose();
    const fitAddon = new FitAddon.FitAddon();

    term = new Terminal({
        cursorBlink: true,
        fontSize: 15,
        fontFamily: 'Menlo, Monaco, Consolas, monospace',
        theme: { background: '#1a1a2e', foreground: '#eee', cursor: '#e94560' }
    });
    term.loadAddon(fitAddon);
    term.open(document.getElementById('terminal'));

    // Fit after a short delay to ensure DOM is ready
    setTimeout(() => {
        fitAddon.fit();
        term.focus();
    }, 200);
    window.addEventListener('resize', () => fitAddon.fit());

    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    ws = new WebSocket(`${proto}//${location.host}/webshell/api/ws/shell?token=${token}`);
    ws.binaryType = 'arraybuffer';

    term.onData(data => {
        if (ws && ws.readyState === WebSocket.OPEN) {
            ws.send(new TextEncoder().encode(data));
        }
    });

    let wsOpened = false;

    ws.onopen = () => {
        wsOpened = true;
        console.log('WebSocket connected');
        term.focus();
    };

    ws.onmessage = (e) => {
        if (e.data instanceof ArrayBuffer) {
            term.write(new Uint8Array(e.data));
        } else {
            term.write(e.data);
        }
    };

    ws.onclose = (e) => {
        console.log('WS closed:', e.code);
        if (!wsOpened) {
            // 连接未成功就断开，说明认证失败（token 过期或无效）
            term.write('\r\n\x1b[31m[认证失败或 Token 已过期，正在返回登录页面...]\x1b[0m\r\n');
            localStorage.removeItem('token');
            localStorage.removeItem('username');
            setTimeout(() => {
                if (term) { term.dispose(); term = null; }
                document.getElementById('auth-page').style.display = 'flex';
                document.getElementById('shell-page').style.display = 'none';
                document.getElementById('login-form').style.display = 'block';
                document.getElementById('change-pw-form').style.display = 'none';
                document.getElementById('form-subtitle').textContent = 'Token 已过期，请重新登录';
                showError('会话已过期，请重新登录');
            }, 2000);
        } else {
            term.write('\r\n\x1b[31m[连接已断开]\x1b[0m\r\n');
        }
    };

    ws.onerror = () => {
        if (!wsOpened) {
            term.write('\r\n\x1b[31m[连接错误，正在返回登录页面...]\x1b[0m\r\n');
            localStorage.removeItem('token');
            localStorage.removeItem('username');
            setTimeout(() => {
                if (term) { term.dispose(); term = null; }
                document.getElementById('auth-page').style.display = 'flex';
                document.getElementById('shell-page').style.display = 'none';
                document.getElementById('login-form').style.display = 'block';
                document.getElementById('change-pw-form').style.display = 'none';
                document.getElementById('form-subtitle').textContent = '连接失败，请重新登录';
                showError('连接失败，请重新登录');
            }, 2000);
        } else {
            term.write('\r\n\x1b[31m[连接错误]\x1b[0m\r\n');
        }
    };
}

// Init
(function() {
    const token = localStorage.getItem('token');
    if (token) {
        startShell();
    }
})();
