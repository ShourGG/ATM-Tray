const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

// ==================== 窗口控制 ====================
const appWindow = getCurrentWindow();

document.getElementById('btn-minimize').addEventListener('click', () => appWindow.minimize());
document.getElementById('btn-maximize').addEventListener('click', async () => {
  if (await appWindow.isMaximized()) {
    appWindow.unmaximize();
  } else {
    appWindow.maximize();
  }
});
document.getElementById('btn-close').addEventListener('click', () => appWindow.close());

// ==================== 状态管理 ====================
let state = {
  isLoggedIn: false,
  tokens: [],
  activeTokenId: null,
  deviceId: '',
  savedCodes: [],
  currentCode: '',
};

// ==================== DOM 元素 ====================
const elements = {
  // 页面
  pageLogin: document.getElementById('page-login'),
  pageMain: document.getElementById('page-main'),
  
  // 登录页
  activationCode: document.getElementById('activation-code'),
  btnActivate: document.getElementById('btn-activate'),
  loginError: document.getElementById('login-error'),
  deviceIdDisplay: document.getElementById('device-id-display'),
  versionDisplay: document.getElementById('version-display'),
  
  // 主页
  quotaBadge: document.getElementById('quota-badge'),
  btnRefresh: document.getElementById('btn-refresh'),
  btnLogout: document.getElementById('btn-logout'),
  tokenList: document.getElementById('token-list'),
  tokenItems: document.getElementById('token-items'),
  loadingState: document.getElementById('loading-state'),
  emptyState: document.getElementById('empty-state'),
  statusText: document.getElementById('status-text'),
  footerVersion: document.getElementById('footer-version'),
  
  // 弹窗
  modalQuota: document.getElementById('modal-quota'),
  quotaInfo: document.getElementById('quota-info'),
  btnCloseModal: document.getElementById('btn-close-modal'),
};

// ==================== 初始化 ====================
async function init() {
  // 获取设备ID
  try {
    state.deviceId = await invoke('get_device_id');
    elements.deviceIdDisplay.textContent = `设备ID: ${state.deviceId.substring(0, 16)}...`;
  } catch (e) {
    console.error('获取设备ID失败:', e);
  }
  
  // 获取版本
  try {
    const info = await invoke('get_app_info');
    elements.versionDisplay.textContent = `v${info.version}`;
    elements.footerVersion.textContent = `v${info.version}`;
  } catch (e) {
    console.error('获取版本失败:', e);
  }
  
  // 检查登录状态
  try {
    const status = await invoke('get_session_status');
    if (status.isLoggedIn) {
      state.isLoggedIn = true;
      showMainPage();
      loadTokens();
    } else {
      // 尝试自动登录：使用上次保存的激活码
      await tryAutoLogin();
    }
  } catch (e) {
    console.error('检查登录状态失败:', e);
    await tryAutoLogin();
  }
  
  // 绑定事件
  bindEvents();
  
  // 启动心跳
  startHeartbeat();
}

// 尝试自动登录
async function tryAutoLogin() {
  try {
    const saved = await invoke('get_saved_codes');
    state.savedCodes = saved.codes || [];
    
    if (saved.lastUsed) {
      setStatus('正在自动登录...');
      elements.activationCode.value = saved.lastUsed;
      
      const result = await invoke('activate_license', { code: saved.lastUsed });
      if (result.success) {
        state.isLoggedIn = true;
        state.currentCode = saved.lastUsed;
        showMainPage();
        loadTokens();
        setStatus('自动登录成功');
        return;
      }
    }
    
    // 显示已保存的激活码列表
    if (state.savedCodes.length > 0) {
      renderSavedCodes();
    }
  } catch (e) {
    console.error('自动登录失败:', e);
  }
}

function bindEvents() {
  // 激活码输入格式化
  elements.activationCode.addEventListener('input', formatActivationCode);
  elements.activationCode.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      handleActivate();
    }
  });
  
  // 激活按钮
  elements.btnActivate.addEventListener('click', handleActivate);
  
  // 刷新按钮
  elements.btnRefresh.addEventListener('click', loadTokens);
  
  // 退出按钮
  elements.btnLogout.addEventListener('click', handleLogout);
  
  // 添加激活码按钮
  document.getElementById('btn-add-code').addEventListener('click', showAddCodeModal);
  document.getElementById('btn-close-add-code').addEventListener('click', closeAddCodeModal);
  document.getElementById('modal-add-code').querySelector('.modal-backdrop').addEventListener('click', closeAddCodeModal);
  document.getElementById('btn-confirm-add-code').addEventListener('click', handleAddCode);
  document.getElementById('new-code-input').addEventListener('input', formatActivationCode);
  document.getElementById('new-code-input').addEventListener('keydown', (e) => {
    if (e.key === 'Enter') handleAddCode();
  });
  
  // 激活码管理面板
  document.getElementById('btn-toggle-codes').addEventListener('click', toggleCodesPanel);
  document.getElementById('codes-panel-header').addEventListener('click', toggleCodesPanel);
  
  // 退出弹窗
  document.getElementById('logout-backdrop').addEventListener('click', closeLogoutModal);
  document.getElementById('btn-close-logout').addEventListener('click', closeLogoutModal);
  document.getElementById('btn-logout-keep').addEventListener('click', logoutKeepCodes);
  document.getElementById('btn-logout-clear').addEventListener('click', logoutClearAll);
  document.getElementById('btn-logout-cancel').addEventListener('click', closeLogoutModal);
  
  // 关闭弹窗
  elements.btnCloseModal.addEventListener('click', closeModal);
  elements.modalQuota.querySelector('.modal-backdrop').addEventListener('click', closeModal);
}

// ==================== 激活码格式化 ====================
function formatActivationCode(e) {
  let value = e.target.value.toUpperCase().replace(/[^A-Z0-9]/g, '');
  let formatted = '';
  for (let i = 0; i < value.length && i < 16; i++) {
    if (i > 0 && i % 4 === 0) {
      formatted += '-';
    }
    formatted += value[i];
  }
  e.target.value = formatted;
}

// ==================== 激活处理 ====================
async function handleActivate() {
  const code = elements.activationCode.value.trim();
  if (!code) {
    showLoginError('请输入激活码');
    return;
  }
  
  setActivateLoading(true);
  hideLoginError();
  
  try {
    const result = await invoke('activate_license', { code });
    
    if (result.success) {
      state.isLoggedIn = true;
      state.currentCode = code;
      elements.quotaBadge.textContent = `配额: ${result.quota || '-'}`;
      showMainPage();
      loadTokens();
    } else {
      showLoginError(result.error || '激活失败');
    }
  } catch (e) {
    showLoginError(`激活失败: ${e}`);
  } finally {
    setActivateLoading(false);
  }
}

// 渲染已保存的激活码列表
function renderSavedCodes() {
  if (state.savedCodes.length === 0) return;
  
  const container = document.querySelector('.login-form');
  let savedCodesHtml = document.getElementById('saved-codes-container');
  
  if (!savedCodesHtml) {
    savedCodesHtml = document.createElement('div');
    savedCodesHtml.id = 'saved-codes-container';
    savedCodesHtml.className = 'saved-codes';
    container.insertBefore(savedCodesHtml, container.firstChild);
  }
  
  savedCodesHtml.innerHTML = `
    <div class="saved-codes-header">已保存的激活码</div>
    <div class="saved-codes-list">
      ${state.savedCodes.map(code => `
        <div class="saved-code-item">
          <span class="saved-code-text">${code}</span>
          <div class="saved-code-actions">
            <button class="btn btn-sm btn-ghost" onclick="useSavedCode('${code}')">使用</button>
            <button class="btn btn-sm btn-ghost btn-danger" onclick="removeSavedCode('${code}')">删除</button>
          </div>
        </div>
      `).join('')}
    </div>
  `;
}

// 使用保存的激活码
async function useSavedCode(code) {
  elements.activationCode.value = code;
  await handleActivate();
}

// 删除保存的激活码
async function removeSavedCode(code) {
  try {
    await invoke('remove_saved_code', { code });
    state.savedCodes = state.savedCodes.filter(c => c !== code);
    renderSavedCodes();
  } catch (e) {
    console.error('删除激活码失败:', e);
  }
}

// ==================== 激活码管理面板 ====================
function toggleCodesPanel() {
  const body = document.getElementById('codes-panel-body');
  const btn = document.getElementById('btn-toggle-codes');
  const isOpen = body.style.display !== 'none';
  
  body.style.display = isOpen ? 'none' : 'block';
  btn.innerHTML = isOpen 
    ? '<svg class="icon" viewBox="0 0 24 24"><path d="M6 9l6 6 6-6" stroke="currentColor" stroke-width="2" fill="none"/></svg>'
    : '<svg class="icon" viewBox="0 0 24 24"><path d="M18 15l-6-6-6 6" stroke="currentColor" stroke-width="2" fill="none"/></svg>';
  
  if (!isOpen) {
    renderCodesList();
  }
}

async function renderCodesList() {
  const list = document.getElementById('codes-list');
  
  try {
    const saved = await invoke('get_saved_codes');
    const codes = saved.codes || [];
    
    if (codes.length === 0) {
      list.innerHTML = '<div style="color:var(--text-muted);font-size:13px;text-align:center;padding:10px;">暂无激活码</div>';
      return;
    }
    
    list.innerHTML = codes.map(code => `
      <div class="code-item">
        <span class="code-item-text">${code}</span>
        <div class="code-item-actions">
          <button class="btn btn-sm btn-ghost btn-danger" onclick="deleteCode('${code}')">删除</button>
        </div>
      </div>
    `).join('');
  } catch (e) {
    list.innerHTML = '<div style="color:var(--danger);font-size:13px;">加载失败</div>';
  }
}

async function deleteCode(code) {
  if (!confirm(`确定删除激活码 ${code} 吗？\n删除后该激活码的账号将不再显示。`)) {
    return;
  }
  
  try {
    await invoke('remove_saved_code', { code });
    renderCodesList();
    loadTokens(); // 重新加载账号列表
    setStatus(`已删除激活码: ${code}`);
  } catch (e) {
    setStatus(`删除失败: ${e}`);
  }
}

// ==================== 添加激活码弹窗 ====================
function showAddCodeModal() {
  document.getElementById('new-code-input').value = '';
  document.getElementById('add-code-error').style.display = 'none';
  document.getElementById('modal-add-code').style.display = 'flex';
}

function closeAddCodeModal() {
  document.getElementById('modal-add-code').style.display = 'none';
}

async function handleAddCode() {
  const input = document.getElementById('new-code-input');
  const code = input.value.trim();
  const errorEl = document.getElementById('add-code-error');
  const btn = document.getElementById('btn-confirm-add-code');
  
  if (!code) {
    errorEl.textContent = '请输入激活码';
    errorEl.style.display = 'block';
    return;
  }
  
  // 设置加载状态
  btn.disabled = true;
  btn.querySelector('.btn-text').style.display = 'none';
  btn.querySelector('.btn-loading').style.display = 'flex';
  errorEl.style.display = 'none';
  
  try {
    const result = await invoke('activate_license', { code });
    
    if (result.success) {
      closeAddCodeModal();
      setStatus(`已添加激活码: ${code}`);
      loadTokens(); // 重新加载账号列表
    } else {
      errorEl.textContent = result.error || '添加失败';
      errorEl.style.display = 'block';
    }
  } catch (e) {
    errorEl.textContent = `添加失败: ${e}`;
    errorEl.style.display = 'block';
  } finally {
    btn.disabled = false;
    btn.querySelector('.btn-text').style.display = '';
    btn.querySelector('.btn-loading').style.display = 'none';
  }
}

function setActivateLoading(loading) {
  const btn = elements.btnActivate;
  const text = btn.querySelector('.btn-text');
  const spinner = btn.querySelector('.btn-loading');
  
  btn.disabled = loading;
  text.style.display = loading ? 'none' : '';
  spinner.style.display = loading ? 'flex' : 'none';
}

function showLoginError(msg) {
  elements.loginError.textContent = msg;
  elements.loginError.style.display = 'block';
}

function hideLoginError() {
  elements.loginError.style.display = 'none';
}

// ==================== 页面切换 ====================
function showMainPage() {
  elements.pageLogin.style.display = 'none';
  elements.pageMain.style.display = 'flex';
}

function showLoginPage() {
  elements.pageMain.style.display = 'none';
  elements.pageLogin.style.display = 'flex';
  elements.activationCode.value = '';
  hideLoginError();
}

// ==================== Token 列表 ====================
async function loadTokens() {
  showLoading();
  setStatus('加载中...');
  
  try {
    // 使用 get_all_tokens 获取所有激活码的账号
    const result = await invoke('get_all_tokens');
    
    if (result.success) {
      state.tokens = result.data || [];
      renderTokens();
      updateQuotaBadge();
      setStatus(`已加载 ${state.tokens.length} 个账号`);
      startQuotaAutoRefresh(); // 启动余额自动刷新
    } else {
      if (result.error === '无有效会话' || result.error?.includes('SESSION_EXPIRED')) {
        handleSessionExpired();
      } else {
        showEmpty();
        setStatus(`加载失败: ${result.error}`);
      }
    }
  } catch (e) {
    showEmpty();
    setStatus(`加载失败: ${e}`);
  }
}

function showLoading() {
  elements.loadingState.style.display = 'flex';
  elements.emptyState.style.display = 'none';
  elements.tokenItems.innerHTML = '';
}

function showEmpty() {
  elements.loadingState.style.display = 'none';
  elements.emptyState.style.display = 'flex';
  elements.tokenItems.innerHTML = '';
}

function renderTokens() {
  elements.loadingState.style.display = 'none';
  
  if (state.tokens.length === 0) {
    showEmpty();
    return;
  }
  
  elements.emptyState.style.display = 'none';
  elements.tokenItems.innerHTML = state.tokens.map(token => {
    const used = token.quota_used || 0;
    const total = token.quota_total || 0;
    const remaining = total - used;
    const quotaText = total > 0 ? formatQuota(remaining) + '/' + formatQuota(total) : '未查询';
    const quotaClass = remaining <= 0 ? 'danger' : (remaining < 100000 ? 'warning' : 'success');
    
    return `
    <div class="token-item ${token.id === state.activeTokenId ? 'active' : ''}" data-id="${token.id}">
      <div class="token-info">
        <div class="token-email">${escapeHtml(token.email || '未知账号')}</div>
        <div class="token-meta">
          <span class="token-status">
            <span class="status-dot ${token.is_valid ? 'valid' : 'invalid'}"></span>
            ${token.is_valid ? '正常' : '异常'}
          </span>
          ${token.name ? `<span>· ${escapeHtml(token.name)}</span>` : ''}
          <span class="token-quota ${quotaClass}">余额: ${quotaText}</span>
        </div>
      </div>
      <div class="token-actions">
        <button class="btn btn-sm btn-ghost btn-view-quota" data-id="${token.id}">刷新余额</button>
        ${token.id === state.activeTokenId 
          ? `<button class="btn btn-sm btn-activated" disabled>已登录</button>`
          : `<button class="btn btn-sm btn-success btn-activate-token" data-id="${token.id}">激活</button>`
        }
      </div>
    </div>
  `}).join('');
  
  // 绑定事件
  elements.tokenItems.querySelectorAll('.btn-view-quota').forEach(btn => {
    btn.addEventListener('click', () => viewQuota(btn.dataset.id));
  });
  
  elements.tokenItems.querySelectorAll('.btn-activate-token').forEach(btn => {
    btn.addEventListener('click', () => activateToken(btn.dataset.id));
  });
}

// ==================== 激活 Token ====================
async function activateToken(tokenId) {
  const token = state.tokens.find(t => t.id === tokenId);
  if (!token) return;
  
  setStatus(`正在激活 ${token.email || '账号'}...`);
  
  try {
    const result = await invoke('activate_token', { tokenId });
    
    if (result.success) {
      state.activeTokenId = tokenId;
      renderTokens();
      setStatus(`已激活: ${token.email || '账号'}`);
    } else {
      if (result.error === 'SESSION_EXPIRED') {
        handleSessionExpired();
      } else {
        setStatus(`激活失败: ${result.error}`);
      }
    }
  } catch (e) {
    setStatus(`激活失败: ${e}`);
  }
}

// ==================== 更新配额显示 ====================
function updateQuotaBadge() {
  const count = state.tokens.length;
  elements.quotaBadge.textContent = `账号: ${count}`;
}

// ==================== 查看余额 ====================
async function viewQuota(tokenId) {
  const token = state.tokens.find(t => t.id === tokenId);
  if (!token) return;
  
  elements.modalQuota.style.display = 'flex';
  elements.quotaInfo.innerHTML = `
    <div class="loading-state">
      <svg class="spinner" viewBox="0 0 24 24"><circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" fill="none" stroke-dasharray="30 70"></circle></svg>
      <p>查询中...</p>
    </div>
  `;
  
  try {
    // 重新加载 token 列表获取最新余额
    const result = await invoke('get_all_tokens');
    
    if (result.success) {
      state.tokens = result.data || [];
      const updatedToken = state.tokens.find(t => t.id === tokenId);
      
      if (updatedToken) {
        const used = updatedToken.quota_used || 0;
        const total = updatedToken.quota_total || 0;
        const remaining = Math.max(0, total - used);
        
        elements.quotaInfo.innerHTML = `
          <div class="quota-row">
            <span class="quota-label">账号</span>
            <span class="quota-value">${escapeHtml(updatedToken.email || '-')}</span>
          </div>
          <div class="quota-row">
            <span class="quota-label">已使用</span>
            <span class="quota-value">${formatQuota(used)}</span>
          </div>
          <div class="quota-row">
            <span class="quota-label">总配额</span>
            <span class="quota-value">${formatQuota(total)}</span>
          </div>
          <div class="quota-row">
            <span class="quota-label">剩余</span>
            <span class="quota-value highlight">${formatQuota(remaining)}</span>
          </div>
        `;
        
        renderTokens(); // 更新列表显示
      } else {
        elements.quotaInfo.innerHTML = `<div class="error-message">账号不存在</div>`;
      }
    } else {
      elements.quotaInfo.innerHTML = `
        <div class="error-message">查询失败: ${result.error || '未知错误'}</div>
      `;
    }
  } catch (e) {
    elements.quotaInfo.innerHTML = `
      <div class="error-message">查询失败: ${e}</div>
    `;
  }
}

function closeModal() {
  elements.modalQuota.style.display = 'none';
}

// ==================== 退出登录 ====================
async function handleLogout() {
  showLogoutModal();
}

function showLogoutModal() {
  const modal = document.getElementById('modal-logout');
  if (modal) {
    modal.style.display = 'flex';
  }
}

function closeLogoutModal() {
  const modal = document.getElementById('modal-logout');
  if (modal) {
    modal.style.display = 'none';
  }
}

async function logoutKeepCodes() {
  closeLogoutModal();
  stopQuotaAutoRefresh(); // 停止余额自动刷新
  try {
    await invoke('logout');
  } catch (e) {
    console.error('退出失败:', e);
  }
  
  state.isLoggedIn = false;
  state.tokens = [];
  state.activeTokenId = null;
  showLoginPage();
  setStatus('已退出，激活码已保留');
}

async function logoutClearAll() {
  closeLogoutModal();
  stopQuotaAutoRefresh(); // 停止余额自动刷新
  try {
    // 清除所有保存的激活码
    const saved = await invoke('get_saved_codes');
    for (const code of (saved.codes || [])) {
      await invoke('remove_saved_code', { code });
    }
    await invoke('logout');
  } catch (e) {
    console.error('退出失败:', e);
  }
  
  state.isLoggedIn = false;
  state.tokens = [];
  state.activeTokenId = null;
  state.savedCodes = [];
  showLoginPage();
  setStatus('已退出并清除所有数据');
}

// ==================== 会话过期 ====================
async function handleSessionExpired() {
  stopQuotaAutoRefresh(); // 停止余额自动刷新
  state.isLoggedIn = false;
  state.tokens = [];
  
  // 尝试自动重新登录
  try {
    const saved = await invoke('get_saved_codes');
    if (saved.lastUsed) {
      setStatus('会话已过期，正在重新登录...');
      const result = await invoke('activate_license', { code: saved.lastUsed });
      if (result.success) {
        state.isLoggedIn = true;
        state.currentCode = saved.lastUsed;
        showMainPage();
        loadTokens();
        setStatus('已自动重新登录');
        return;
      }
    }
  } catch (e) {
    console.error('自动重新登录失败:', e);
  }
  
  // 自动登录失败，显示登录页
  showLoginPage();
  showLoginError('会话已过期，请重新激活');
}

// ==================== 心跳 ====================
function startHeartbeat() {
  // 心跳检查
  setInterval(async () => {
    if (!state.isLoggedIn) return;
    
    try {
      const result = await invoke('heartbeat');
      if (!result.valid) {
        handleSessionExpired();
      }
    } catch (e) {
      console.error('心跳失败:', e);
    }
  }, 60000); // 每分钟
  
  // 自动检查服务器 token 版本（每 5 分钟检查一次，只有版本变化才会下载）
  setInterval(async () => {
    if (!state.isLoggedIn) return;
    
    try {
      const result = await invoke('refresh_active_token', { force: false });
      if (result.refreshed) {
        console.log('[Token] 服务器 token 已更新，本地已同步');
        setStatus('Token 已自动同步');
      }
    } catch (e) {
      console.error('检查 token 版本失败:', e);
    }
  }, 5 * 60 * 1000); // 每 5 分钟检查一次服务器版本
  
  // 启动时立即检查一次
  setTimeout(async () => {
    if (!state.isLoggedIn) return;
    try {
      const result = await invoke('refresh_active_token', { force: false });
      if (result.refreshed) {
        console.log('[Token] 启动时同步了服务器最新 token');
      }
    } catch (e) {
      console.error('启动检查 token 失败:', e);
    }
  }, 3000);
}

// ==================== 余额自动刷新 ====================
let quotaRefreshTimer = null;

function startQuotaAutoRefresh() {
  if (quotaRefreshTimer) {
    clearInterval(quotaRefreshTimer);
  }
  
  quotaRefreshTimer = setInterval(async () => {
    if (!state.isLoggedIn || state.tokens.length === 0) return;
    
    // 刷新所有账号的余额
    for (const token of state.tokens) {
      try {
        const result = await invoke('get_subscription', { tokenId: token.id });
        if (result.success && result.data) {
          token.quota_used = result.data.premiumUsage || 0;
          token.quota_total = result.data.premiumLimit || 500;
        }
      } catch (e) {
        console.error(`刷新余额失败 ${token.email}:`, e);
      }
    }
    
    renderTokens();
    setStatus(`余额已刷新 (${new Date().toLocaleTimeString()})`);
  }, 30000); // 每30秒
}

function stopQuotaAutoRefresh() {
  if (quotaRefreshTimer) {
    clearInterval(quotaRefreshTimer);
    quotaRefreshTimer = null;
  }
}

// ==================== 工具函数 ====================
function setStatus(text) {
  elements.statusText.textContent = text;
}

function formatQuota(num) {
  if (num >= 1000000) {
    return (num / 1000000).toFixed(1) + 'M';
  } else if (num >= 1000) {
    return (num / 1000).toFixed(1) + 'K';
  }
  return num.toString();
}

function escapeHtml(str) {
  if (!str) return '';
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

// ==================== 暴露全局函数 ====================
window.deleteCode = deleteCode;
window.useSavedCode = useSavedCode;
window.removeSavedCode = removeSavedCode;
window.closeLogoutModal = closeLogoutModal;
window.logoutKeepCodes = logoutKeepCodes;
window.logoutClearAll = logoutClearAll;

// ==================== 启动 ====================
document.addEventListener('DOMContentLoaded', init);
