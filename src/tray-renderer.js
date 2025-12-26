const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;
const { listen } = window.__TAURI__.event;

const appWindow = getCurrentWindow();
const APP_VERSION = '2.2.3';

// ==================== 全局错误处理 ====================
// 捕获未处理的错误，防止黑屏
window.onerror = function(msg, url, line, col, error) {
  console.error('[GlobalError]', msg, url, line, col, error);
  ensurePageVisible();
  return false;
};

window.onunhandledrejection = function(event) {
  console.error('[UnhandledRejection]', event.reason);
  ensurePageVisible();
};

// 确保页面可见（防止黑屏）
function ensurePageVisible() {
  const pageLogin = document.getElementById('page-login');
  const pageMain = document.getElementById('page-main');
  const pageSelect = document.getElementById('page-mode-select');
  
  // 检查是否所有页面都不可见
  const loginVisible = pageLogin && (pageLogin.classList.contains('page-active') || pageLogin.style.display !== 'none');
  const mainVisible = pageMain && pageMain.classList.contains('page-active');
  const selectVisible = pageSelect && pageSelect.classList.contains('page-active');
  
  if (!loginVisible && !mainVisible && !selectVisible) {
    // 所有页面都不可见，强制显示登录页面
    if (pageLogin) {
      pageLogin.style.display = '';
      pageLogin.classList.remove('page-inactive');
      pageLogin.classList.add('page-active');
    }
    if (pageMain) {
      pageMain.style.display = 'none';
      pageMain.classList.remove('page-active');
      pageMain.classList.add('page-inactive');
    }
    if (pageSelect) {
      pageSelect.style.display = 'none';
      pageSelect.classList.remove('page-active');
      pageSelect.classList.add('page-inactive');
    }
  }
}

// ==================== 安全防护 ====================
(function() {
  // 禁用右键菜单
  document.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    return false;
  });
  
  // 禁用开发者工具快捷键
  document.addEventListener('keydown', (e) => {
    // F12
    if (e.key === 'F12') {
      e.preventDefault();
      return false;
    }
    // Ctrl+Shift+I (开发者工具)
    if (e.ctrlKey && e.shiftKey && e.key === 'I') {
      e.preventDefault();
      return false;
    }
    // Ctrl+Shift+J (控制台)
    if (e.ctrlKey && e.shiftKey && e.key === 'J') {
      e.preventDefault();
      return false;
    }
    // Ctrl+Shift+C (检查元素)
    if (e.ctrlKey && e.shiftKey && e.key === 'C') {
      e.preventDefault();
      return false;
    }
    // Ctrl+U (查看源代码)
    if (e.ctrlKey && e.key === 'u') {
      e.preventDefault();
      return false;
    }
    // Ctrl+S (保存)
    if (e.ctrlKey && e.key === 's') {
      e.preventDefault();
      return false;
    }
    // Ctrl+P (打印)
    if (e.ctrlKey && e.key === 'p') {
      e.preventDefault();
      return false;
    }
    // F5 / Ctrl+R (刷新)
    if (e.key === 'F5' || (e.ctrlKey && e.key === 'r')) {
      e.preventDefault();
      return false;
    }
    // Ctrl+Shift+R (强制刷新)
    if (e.ctrlKey && e.shiftKey && e.key === 'R') {
      e.preventDefault();
      return false;
    }
    // Alt+Left/Right (前进后退)
    if (e.altKey && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
      e.preventDefault();
      return false;
    }
  });
  
  // 禁用拖放
  document.addEventListener('dragstart', (e) => {
    e.preventDefault();
    return false;
  });
  
  // 禁用选择文本（可选，如果需要可以注释掉）
  // document.addEventListener('selectstart', (e) => {
  //   e.preventDefault();
  //   return false;
  // });
  
  // 开发者工具检测已移除（在Tauri应用中容易误判导致黑屏）
  // 反调试保护已在后端 security.rs 实现
  
  // 清除控制台
  if (typeof console !== 'undefined') {
    const noop = () => {};
    // Release 模式下禁用 console
    if (!window.__TAURI_DEBUG__) {
      console.log = noop;
      console.warn = noop;
      console.error = noop;
      console.info = noop;
      console.debug = noop;
      console.table = noop;
      console.trace = noop;
    }
  }
})();

// 设置
let settings = {
  closeBehavior: localStorage.getItem('closeBehavior') || 'hide'
};

// 状态
let state = {
  isLoggedIn: false,
  tokens: [],
  activeTokenId: null,
  autoSwitch: false, // 是否启用自动切换
  currentMode: 'normal', // 当前模式: 'normal' 或 'autoswitch'
  hasBothLicenses: false, // 是否同时拥有两种激活码
};

// DOM 元素（延迟初始化）
let elements = {};

// 初始化
async function init() {
  // 初始化 DOM 元素
  elements = {
    pageLogin: document.getElementById('page-login'),
    pageMain: document.getElementById('page-main'),
    activationCode: document.getElementById('activation-code'),
    btnActivate: document.getElementById('btn-activate'),
    loginError: document.getElementById('login-error'),
    accountEmail: document.getElementById('account-email'),
    accountQuota: document.getElementById('account-quota'),
    statusDot: document.getElementById('status-dot'),
    statusLabel: document.getElementById('status-label'),
    tokenItems: document.getElementById('token-items'),
    statusText: document.getElementById('status-text'),
    btnRefresh: document.getElementById('btn-refresh'),
    btnLogout: document.getElementById('btn-logout'),
    codeList: document.getElementById('code-list'),
    btnAddCode: document.getElementById('btn-add-code'),
    newActivationCode: document.getElementById('new-activation-code'),
    btnConfirmAddCode: document.getElementById('btn-confirm-add-code'),
    addCodeError: document.getElementById('add-code-error'),
    
    // 方案3 新增元素
    drawer: document.getElementById('main-drawer'),
    drawerBackdrop: document.getElementById('drawer-backdrop'),
    btnOpenDrawer: document.getElementById('btn-open-drawer'),
    quotaValue: document.getElementById('quota-value'),
    quotaProgress: document.getElementById('quota-progress'),
    quotaTotal: document.getElementById('quota-total'),
    drawerHandle: document.querySelector('.drawer-handle-bar'),
    activationArrow: document.getElementById('activation-arrow'),
    versionBadge: document.getElementById('version-badge'),
  };
  
  // 加载版本号
  try {
    const appInfo = await invoke('get_app_info');
    if (appInfo.version && elements.versionBadge) {
      elements.versionBadge.textContent = 'v' + appInfo.version;
    }
  } catch (e) {
    console.error('获取版本号失败:', e);
  }
  
  // 移除内联样式，使用类名控制
  elements.pageLogin.style.display = '';
  elements.pageMain.style.display = '';
  elements.pageLogin.classList.add('page-active');
  elements.pageMain.classList.add('page-inactive');
  
  bindEvents();
  setupDrawerInteractions(); // 初始化抽屉交互
  setupCodeListDelegation(); // 初始化激活码列表事件委托
  setupTokenListDelegation(); // 初始化 Token 列表事件委托
  
  try {
    // 先检查激活码状态
    const licenseStatus = await invoke('check_license_status');
    state.hasBothLicenses = licenseStatus.hasBoth;
    state.currentMode = licenseStatus.currentMode || 'normal';
    
    const status = await invoke('get_session_status');
    if (status.isLoggedIn) {
      state.isLoggedIn = true;
      // 如果有两种激活码，显示模式选择界面
      if (state.hasBothLicenses) {
        showModeSelectPage(licenseStatus);
      } else {
        showMainPage();
        loadTokens();
      }
    } else {
      await tryAutoLogin();
    }
  } catch (e) {
    console.error('初始化失败:', e);
    await tryAutoLogin();
  }
  
  startHeartbeat();
  startAutoRefresh();
  
  // 启动 WebSocket 实时同步
  connectWebSocket();
  startWsHeartbeat();
  
  // 获取自动切换状态
  initAutoSwitchState();
  
  // 首次启动默认开启自启动
  initAutostart();
  
  // 首次启动自动显示公告
  checkAutoShowNotice();
}

// 获取自动切换状态
async function initAutoSwitchState() {
  try {
    const result = await invoke('get_auto_switch_status');
    state.autoSwitch = result.enabled || false;
    console.log('[AutoSwitch] 状态:', state.autoSwitch ? '已启用' : '未启用');
  } catch (e) {
    console.error('[AutoSwitch] 获取状态失败:', e);
    state.autoSwitch = false;
  }
}

// ==================== 双激活码模式管理 ====================

// 显示模式选择页面
function showModeSelectPage(licenseStatus) {
  const pageSelect = document.getElementById('page-mode-select');
  const pageLogin = document.getElementById('page-login');
  const pageMain = document.getElementById('page-main');
  
  if (!pageSelect) {
    console.error('模式选择页面不存在');
    // 降级处理：直接进入主页面
    showMainPage();
    loadTokens();
    return;
  }
  
  // 隐藏其他页面
  pageLogin.style.display = 'none';
  pageLogin.classList.remove('page-active');
  pageLogin.classList.add('page-inactive');
  pageMain.style.display = 'none';
  pageMain.classList.remove('page-active');
  pageMain.classList.add('page-inactive');
  
  // 显示模式选择页面
  pageSelect.style.display = '';  // 清除 display:none
  pageSelect.classList.remove('page-inactive');
  pageSelect.classList.add('page-active');
  
  // 显示激活码信息
  const normalCode = document.getElementById('normal-license-code');
  const autoswitchCode = document.getElementById('autoswitch-license-code');
  if (normalCode) normalCode.textContent = maskCode(licenseStatus.normalCode);
  if (autoswitchCode) autoswitchCode.textContent = maskCode(licenseStatus.autoswitchCode);
}

// 隐藏激活码中间部分
function maskCode(code) {
  if (!code || code.length < 8) return code || '';
  return code.substring(0, 4) + '****' + code.substring(code.length - 4);
}

// 选择模式并进入
async function selectMode(mode) {
  try {
    showLoadingOverlay('正在切换模式...');
    
    // 获取对应模式的激活码
    const licenseResult = await invoke('get_license_code', { mode });
    if (!licenseResult.success) {
      hideLoadingOverlay();
      showToast('error', '获取激活码失败', 2000);
      return;
    }
    
    // 用对应激活码重新激活，这样会话就切换到该激活码
    const activateResult = await invoke('activate_license', { code: licenseResult.code });
    if (!activateResult.success) {
      hideLoadingOverlay();
      showToast('error', '激活失败: ' + (activateResult.error || '未知错误'), 2000);
      return;
    }
    
    // 设置当前模式
    const result = await invoke('set_current_mode', { mode });
    if (result.success) {
      state.currentMode = mode;
      state.autoSwitch = (mode === 'autoswitch');
      
      // 隐藏模式选择页面
      const pageSelect = document.getElementById('page-mode-select');
      if (pageSelect) {
        pageSelect.style.display = 'none';
        pageSelect.classList.remove('page-active');
        pageSelect.classList.add('page-inactive');
      }
      
      hideLoadingOverlay();
      
      // 显示主页面
      showMainPage();
      loadTokens();
      
      // 显示模式指示器
      updateModeIndicator();
      
      showToast('success', `已进入${mode === 'autoswitch' ? '自动切换' : '正常'}模式`, 2000);
    }
  } catch (e) {
    console.error('设置模式失败:', e);
    hideLoadingOverlay();
    showToast('error', '设置模式失败', 2000);
  }
}

// 更新模式指示器
function updateModeIndicator() {
  const indicator = document.getElementById('mode-indicator');
  if (indicator) {
    if (state.currentMode === 'autoswitch') {
      indicator.textContent = '自动切换';
      indicator.style.display = 'inline-block';
      indicator.className = 'mode-indicator autoswitch';
    } else if (state.hasBothLicenses) {
      indicator.textContent = '正常模式';
      indicator.style.display = 'inline-block';
      indicator.className = 'mode-indicator normal';
    } else {
      indicator.style.display = 'none';
    }
  }
}

// 切换模式（从设置中调用）
async function switchMode() {
  if (!state.hasBothLicenses) {
    showToast('info', '只有一种激活码，无需切换', 2000);
    return;
  }
  
  const newMode = state.currentMode === 'autoswitch' ? 'normal' : 'autoswitch';
  await selectMode(newMode);
  showToast('success', `已切换到${newMode === 'autoswitch' ? '自动切换' : '正常'}模式`, 2000);
}

// 首次启动默认启用自启动，并更新自启动路径
async function initAutostart() {
  const hasInitAutostart = localStorage.getItem('hasInitAutostart');
  if (!hasInitAutostart) {
    try {
      await invoke('set_autostart', { enabled: true });
      localStorage.setItem('hasInitAutostart', 'true');
      console.log('[Autostart] 首次启动，已默认开启自启动');
    } catch (e) {
      console.error('[Autostart] 设置自启动失败:', e);
    }
  } else {
    // 每次启动时更新自启动路径（防止exe文件名变化导致自启动失效）
    try {
      await invoke('update_autostart_path');
      console.log('[Autostart] 自启动路径已更新');
    } catch (e) {
      console.error('[Autostart] 更新自启动路径失败:', e);
    }
  }
}

// 自动登录：检查本地是否有有效会话，有则进入主页面（或模式选择）
async function tryAutoLogin() {
  setStatus('正在连接服务器...');
  showLoadingOverlay('正在连接服务器...');
  
  try {
    // 先检查是否有本地保存的有效会话
    const tokensResult = await invoke('get_all_tokens');
    if (tokensResult.success && tokensResult.data && tokensResult.data.length > 0) {
      // 有有效会话
      state.isLoggedIn = true;
      state.tokens = tokensResult.data;
      hideLoadingOverlay();
      
      // 检查是否有两种激活码
      const licenseStatus = await invoke('check_license_status');
      state.hasBothLicenses = licenseStatus.hasBoth;
      state.currentMode = licenseStatus.currentMode || 'normal';
      
      if (state.hasBothLicenses) {
        showModeSelectPage(licenseStatus);
      } else {
        showMainPage();
        renderTokens();
        updateCurrentAccount();
      }
      setStatus(`${state.tokens.length} 个账号`);
      return;
    }
    
    // 没有有效会话，尝试用保存的激活码重新激活
    const saved = await invoke('get_saved_codes');
    if (saved.codes && saved.codes.length > 0) {
      setStatus('正在自动登录...');
      showLoadingOverlay('正在自动登录...');
      // 并行激活所有保存的激活码
      await Promise.all(saved.codes.map(code => 
        invoke('activate_license', { code }).catch(() => null)
      ));
      // 再次检查
      const retry = await invoke('get_all_tokens');
      if (retry.success && retry.data && retry.data.length > 0) {
        state.isLoggedIn = true;
        state.tokens = retry.data;
        hideLoadingOverlay();
        
        // 检查是否有两种激活码
        const licenseStatus = await invoke('check_license_status');
        state.hasBothLicenses = licenseStatus.hasBoth;
        state.currentMode = licenseStatus.currentMode || 'normal';
        
        if (state.hasBothLicenses) {
          showModeSelectPage(licenseStatus);
        } else {
          showMainPage();
          renderTokens();
          updateCurrentAccount();
        }
        setStatus(`${state.tokens.length} 个账号`);
        return;
      }
    }
    hideLoadingOverlay();
    showLoginPage(); // 确保显示登录页面
    setStatus('请输入激活码');
  } catch (e) {
    console.error('自动登录失败:', e);
    hideLoadingOverlay();
    showLoginPage(); // 确保显示登录页面
    setStatus('连接失败，请重试');
  }
}

// 显示登录页面
function showLoginPage() {
  const pageLogin = document.getElementById('page-login');
  const pageMain = document.getElementById('page-main');
  const pageSelect = document.getElementById('page-mode-select');
  
  // 隐藏其他页面
  if (pageMain) {
    pageMain.style.display = 'none';
    pageMain.classList.remove('page-active');
    pageMain.classList.add('page-inactive');
  }
  if (pageSelect) {
    pageSelect.style.display = 'none';
    pageSelect.classList.remove('page-active');
    pageSelect.classList.add('page-inactive');
  }
  
  // 显示登录页面
  if (pageLogin) {
    pageLogin.style.display = '';
    pageLogin.classList.remove('page-inactive');
    pageLogin.classList.add('page-active');
  }
}

// 事件绑定
function bindEvents() {
  // 关闭按钮
  document.getElementById('btn-close').addEventListener('click', async () => {
    // 首次使用时显示引导弹窗
    if (!localStorage.getItem('closeGuideShown')) {
      document.getElementById('modal-close-guide').style.display = 'flex';
      return;
    }
    
    if (settings.closeBehavior === 'exit') {
      await invoke('exit_app');
    } else {
      await invoke('hide_window');
    }
  });
  
  // 首次关闭引导弹窗 - 选择托盘
  document.getElementById('btn-guide-tray')?.addEventListener('click', async () => {
    localStorage.setItem('closeGuideShown', 'true');
    settings.closeBehavior = 'hide';
    localStorage.setItem('closeBehavior', 'hide');
    document.getElementById('modal-close-guide').style.display = 'none';
    await invoke('hide_window');
  });
  
  // 首次关闭引导弹窗 - 选择退出
  document.getElementById('btn-guide-exit')?.addEventListener('click', async () => {
    localStorage.setItem('closeGuideShown', 'true');
    settings.closeBehavior = 'exit';
    localStorage.setItem('closeBehavior', 'exit');
    document.getElementById('modal-close-guide').style.display = 'none';
    await invoke('exit_app');
  });
  
  // 最小化按钮
  document.getElementById('btn-minimize').addEventListener('click', async () => {
    await appWindow.minimize();
  });
  
  // 公告按钮
  document.getElementById('btn-notice').addEventListener('click', () => {
    document.getElementById('modal-notice').style.display = 'flex';
  });
  
  // 关闭公告弹窗
  document.getElementById('btn-close-notice').addEventListener('click', closeNotice);
  document.getElementById('notice-backdrop').addEventListener('click', closeNotice);
  
  // 设置按钮
  document.getElementById('btn-settings').addEventListener('click', async () => {
    document.getElementById('modal-settings').style.display = 'flex';
    document.getElementById('close-behavior').value = settings.closeBehavior;
    // 加载自启动状态
    try {
      const result = await invoke('get_autostart_status');
      document.getElementById('autostart-toggle').checked = result.enabled;
    } catch (e) {
      console.error('获取自启动状态失败:', e);
    }
  });
  
  // 关闭设置弹窗
  document.getElementById('btn-close-settings').addEventListener('click', closeSettings);
  document.getElementById('settings-backdrop').addEventListener('click', closeSettings);
  
  // 保存设置
  document.getElementById('close-behavior').addEventListener('change', (e) => {
    settings.closeBehavior = e.target.value;
    localStorage.setItem('closeBehavior', settings.closeBehavior);
  });
  
  // 自启动开关
  document.getElementById('autostart-toggle').addEventListener('change', async (e) => {
    try {
      await invoke('set_autostart', { enabled: e.target.checked });
    } catch (err) {
      console.error('设置自启动失败:', err);
      e.target.checked = !e.target.checked; // 恢复状态
    }
  });

  elements.activationCode.addEventListener('input', formatCode);
  elements.activationCode.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') handleActivate();
  });
  elements.btnActivate.addEventListener('click', handleActivate);
  elements.btnRefresh.addEventListener('click', loadTokens);
  elements.btnLogout.addEventListener('click', () => {
    document.getElementById('modal-exit').style.display = 'flex';
  });
  
  // 退出弹窗
  document.getElementById('btn-close-exit').addEventListener('click', closeExitModal);
  document.getElementById('exit-backdrop').addEventListener('click', closeExitModal);
  document.getElementById('btn-exit-normal').addEventListener('click', handleLogout);
  document.getElementById('btn-exit-clear').addEventListener('click', handleLogoutWithClear);
  document.getElementById('btn-exit-unbind').addEventListener('click', handleUnbindAndExit);
  
  // 删除确认弹窗
  document.getElementById('btn-close-delete').addEventListener('click', closeDeleteModal);
  document.getElementById('delete-backdrop').addEventListener('click', closeDeleteModal);
  document.getElementById('btn-delete-cancel').addEventListener('click', closeDeleteModal);
  document.getElementById('btn-delete-confirm').addEventListener('click', confirmDeleteCode);
  
  // 添加激活码按钮
  elements.btnAddCode.addEventListener('click', () => {
    document.getElementById('modal-add-code').style.display = 'flex';
    elements.newActivationCode.value = '';
    elements.addCodeError.style.display = 'none';
  });
  
  // 关闭添加激活码弹窗
  document.getElementById('btn-close-add-code').addEventListener('click', closeAddCodeModal);
  document.getElementById('add-code-backdrop').addEventListener('click', closeAddCodeModal);
  
  // 新激活码输入格式化
  elements.newActivationCode.addEventListener('input', formatCode);
  elements.newActivationCode.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') handleAddNewCode();
  });
  elements.btnConfirmAddCode.addEventListener('click', handleAddNewCode);
}

function closeSettings() {
  document.getElementById('modal-settings').style.display = 'none';
}

function closeNotice() {
  // 保存"不再显示"选项（记录当前版本号）
  const dontShow = document.getElementById('notice-dont-show');
  if (dontShow && dontShow.checked) {
    localStorage.setItem('noticeDismissedVersion', APP_VERSION);
  }
  document.getElementById('modal-notice').style.display = 'none';
}

// 检查是否需要自动显示公告（每个新版本显示一次）
function checkAutoShowNotice() {
  const dismissedVersion = localStorage.getItem('noticeDismissedVersion');
  // 如果没有记录，或者记录的版本 < 当前版本，则显示公告
  if (!dismissedVersion || dismissedVersion < APP_VERSION) {
    document.getElementById('modal-notice').style.display = 'flex';
    // 新版本自动标记为已显示（不勾选"不再提醒"也只弹一次）
    localStorage.setItem('noticeDismissedVersion', APP_VERSION);
  }
}

function closeAddCodeModal() {
  document.getElementById('modal-add-code').style.display = 'none';
}

function closeExitModal() {
  document.getElementById('modal-exit').style.display = 'none';
}

// 添加新激活码
async function handleAddNewCode() {
  const code = elements.newActivationCode.value.trim();
  if (!code) {
    elements.addCodeError.textContent = '请输入激活码';
    elements.addCodeError.style.display = 'block';
    return;
  }
  
  elements.btnConfirmAddCode.disabled = true;
  elements.btnConfirmAddCode.querySelector('.btn-text').style.display = 'none';
  elements.btnConfirmAddCode.querySelector('.btn-loading').style.display = '';
  elements.addCodeError.style.display = 'none';
  
  try {
    const result = await invoke('activate_license', { code });
    if (result.success) {
      closeAddCodeModal();
      await loadSavedCodes();
      await loadTokens();
      setStatus('激活码添加成功');
    } else {
      elements.addCodeError.textContent = result.error || '激活失败';
      elements.addCodeError.style.display = 'block';
    }
  } catch (e) {
    elements.addCodeError.textContent = '网络错误';
    elements.addCodeError.style.display = 'block';
  } finally {
    elements.btnConfirmAddCode.disabled = false;
    elements.btnConfirmAddCode.querySelector('.btn-text').style.display = '';
    elements.btnConfirmAddCode.querySelector('.btn-loading').style.display = 'none';
  }
}

// 加载已保存的激活码
async function loadSavedCodes() {
  try {
    const result = await invoke('get_saved_codes');
    renderSavedCodes(result.codes || [], result.lastUsed);
  } catch (e) {
    console.error('加载激活码失败:', e);
  }
}

// 渲染激活码列表（使用 dataset + 事件委托，移除 inline onclick 注入面）
function renderSavedCodes(codes, lastUsed) {
  if (!elements.codeList) return;
  
  if (codes.length === 0) {
    elements.codeList.innerHTML = '<div class="empty-hint">暂无激活码</div>';
    return;
  }
  
  // 使用 DOM API 构建元素，避免 innerHTML + 字符串拼接
  elements.codeList.innerHTML = '';
  
  codes.forEach((code, index) => {
    const isActive = code === lastUsed;
    const delay = index * 0.05;
    
    const item = document.createElement('div');
    item.className = `code-item ${isActive ? 'active' : ''} animate-in`;
    item.style.animationDelay = `${delay}s`;
    
    const info = document.createElement('div');
    info.className = 'code-info';
    
    const valueEl = document.createElement('div');
    valueEl.className = 'code-value';
    valueEl.textContent = code; // textContent 自动转义，防 XSS
    
    const statusEl = document.createElement('div');
    statusEl.className = 'code-status';
    statusEl.textContent = isActive ? '当前使用' : '已保存';
    
    info.appendChild(valueEl);
    info.appendChild(statusEl);
    
    const actions = document.createElement('div');
    actions.className = 'code-actions';
    
    if (!isActive) {
      const switchBtn = document.createElement('button');
      switchBtn.className = 'btn btn-sm';
      switchBtn.title = '切换';
      switchBtn.dataset.action = 'switch';
      switchBtn.dataset.code = code;
      switchBtn.innerHTML = '<svg width="12" height="12" viewBox="0 0 24 24"><path d="M5 12h14M12 5l7 7-7 7" stroke="currentColor" stroke-width="2" fill="none"/></svg>';
      actions.appendChild(switchBtn);
    }
    
    const deleteBtn = document.createElement('button');
    deleteBtn.className = 'btn btn-sm btn-delete';
    deleteBtn.title = '删除';
    deleteBtn.dataset.action = 'delete';
    deleteBtn.dataset.code = code;
    deleteBtn.innerHTML = '<svg width="12" height="12" viewBox="0 0 24 24"><path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" stroke="currentColor" stroke-width="2" fill="none"/></svg>';
    actions.appendChild(deleteBtn);
    
    item.appendChild(info);
    item.appendChild(actions);
    elements.codeList.appendChild(item);
  });
}

// 切换激活码
async function handleSwitchCode(code) {
  setStatus('切换中...');
  try {
    const result = await invoke('activate_license', { code });
    if (result.success) {
      await loadSavedCodes();
      await loadTokens();
      setStatus('切换成功');
    } else {
      setStatus(result.error || '切换失败');
    }
  } catch (e) {
    setStatus('切换失败');
  }
}

// 删除激活码 - 显示确认弹窗
let pendingDeleteCode = null;

function handleDeleteCode(code) {
  pendingDeleteCode = code;
  document.getElementById('modal-delete').style.display = 'flex';
}

function closeDeleteModal() {
  document.getElementById('modal-delete').style.display = 'none';
  pendingDeleteCode = null;
}

async function confirmDeleteCode() {
  if (!pendingDeleteCode) return;
  
  const codeToDelete = pendingDeleteCode;
  closeDeleteModal();
  
  try {
    await invoke('remove_saved_code', { code: codeToDelete });
    await loadSavedCodes();
    await loadTokens();
    setStatus('已删除');
  } catch (e) {
    setStatus('删除失败');
  }
}

// 事件委托：处理激活码列表点击（替代 inline onclick）
function setupCodeListDelegation() {
  if (!elements.codeList) return;
  elements.codeList.addEventListener('click', (e) => {
    const btn = e.target.closest('button[data-action]');
    if (!btn) return;
    
    const action = btn.dataset.action;
    const code = btn.dataset.code;
    if (!code) return;
    
    if (action === 'switch') {
      handleSwitchCode(code);
    } else if (action === 'delete') {
      handleDeleteCode(code);
    }
  });
}

// 事件委托：处理 Token 列表点击（替代 inline onclick）
function setupTokenListDelegation() {
  if (!elements.tokenItems) return;
  elements.tokenItems.addEventListener('click', (e) => {
    const btn = e.target.closest('button[data-action="activate"]');
    if (!btn) return;
    
    const tokenId = btn.dataset.tokenId;
    if (tokenId) {
      activateToken(tokenId);
    }
  });
}

// 格式化激活码
function formatCode(e) {
  let value = e.target.value.toUpperCase().replace(/[^A-Z0-9]/g, '');
  let formatted = '';
  for (let i = 0; i < value.length && i < 16; i++) {
    if (i > 0 && i % 4 === 0) formatted += '-';
    formatted += value[i];
  }
  e.target.value = formatted;
}

// 激活
async function handleActivate() {
  const code = elements.activationCode.value.trim();
  if (!code) {
    showError('请输入激活码');
    return;
  }
  
  setLoading(true);
  hideError();
  
  try {
    const result = await invoke('activate_license', { code });
    if (result.success) {
      state.isLoggedIn = true;
      // 更新自动切换状态
      state.autoSwitch = result.autoSwitch || false;
      console.log('[Activate] 自动切换:', state.autoSwitch ? '已启用' : '未启用');
      showMainPage();
      loadTokens();
    } else {
      showError(result.error || '激活失败');
    }
  } catch (e) {
    showError(`激活失败: ${e}`);
  } finally {
    setLoading(false);
  }
}

// 加载 Token 列表
async function loadTokens() {
  setStatus('加载中...');
  
  try {
    const result = await invoke('get_all_tokens');
    if (result.success) {
      state.tokens = result.data || [];
      
      // 如果当前选中的 Token 不在列表中了，重置选中状态
      if (state.activeTokenId && !state.tokens.find(t => t.id === state.activeTokenId)) {
        state.activeTokenId = null;
      }

      renderTokens();
      updateCurrentAccount();
      
      // 自动切换检查（仅在启用自动切换时）
      if (state.autoSwitch) {
        await checkAndAutoSwitch();
      }
      
      if (state.tokens.length > 0) {
        setStatus(`${state.tokens.length} 个账号`);
      } else {
        setStatus('暂无可用账号');
      }
    } else {
      setStatus(`加载失败: ${result.error}`);
    }
  } catch (e) {
    setStatus(`加载失败: ${e}`);
  }
}

// 检查并自动切换 Token（当余额用完时）
// 策略：优先使用余额最少的 Token（用完一个再用下一个）
async function checkAndAutoSwitch() {
  if (state.tokens.length < 2) return;
  
  // 获取所有有余额的 Token，按余额从少到多排序
  const tokensWithQuota = state.tokens
    .map(t => ({
      ...t,
      remaining: (t.quota_total || 0) - (t.quota_used || 0)
    }))
    .filter(t => t.remaining > 0)
    .sort((a, b) => a.remaining - b.remaining); // 余额少的排前面
  
  if (tokensWithQuota.length === 0) {
    console.log('[AutoSwitch] 所有 Token 余额已用完');
    showToast('warning', '所有账号额度已用完', 3000);
    return;
  }
  
  // 检查当前激活的 Token
  const activeToken = state.tokens.find(t => t.id === state.activeTokenId);
  
  // 如果没有激活的 Token，或当前 Token 余额用完，切换到余额最少的
  if (!activeToken) {
    // 没有激活的 Token，激活余额最少的那个
    const target = tokensWithQuota[0];
    console.log('[AutoSwitch] 自动激活余额最少的 Token:', target.email, '余额:', target.remaining);
    await activateToken(target.id);
    showToast('info', `已自动激活 ${target.email}`, 3000);
    return;
  }
  
  const activeRemaining = (activeToken.quota_total || 0) - (activeToken.quota_used || 0);
  
  // 当前 Token 余额用完，切换到下一个余额最少的
  if (activeRemaining <= 0) {
    const target = tokensWithQuota[0];
    console.log('[AutoSwitch] 当前 Token 余额用完，切换到:', target.email, '余额:', target.remaining);
    await activateToken(target.id);
    showToast('info', `已自动切换到 ${target.email}`, 3000);
  }
}

// 渲染 Token 列表（使用 DOM API + 事件委托，移除 inline onclick 注入面）
function renderTokens() {
  if (state.tokens.length === 0) {
    elements.tokenItems.innerHTML = '<div style="text-align:center;color:var(--text-muted);padding:30px;font-size:12px;">暂无可用账号</div>';
    return;
  }
  
  // 使用 DOM API 构建元素，避免 innerHTML + 字符串拼接
  elements.tokenItems.innerHTML = '';
  
  state.tokens.forEach((token, index) => {
    const quota = formatQuota(token.quota_total - token.quota_used) + '/' + formatQuota(token.quota_total);
    const delay = index * 0.05;
    const isActive = token.id === state.activeTokenId;
    
    const item = document.createElement('div');
    item.className = `token-item ${isActive ? 'active' : ''} animate-in`;
    item.dataset.id = token.id;
    item.style.animationDelay = `${delay}s`;
    
    const info = document.createElement('div');
    info.className = 'token-item-info';
    
    const emailEl = document.createElement('span');
    emailEl.className = 'token-item-email';
    emailEl.textContent = token.email || '未知'; // textContent 自动转义
    
    const quotaEl = document.createElement('span');
    quotaEl.className = 'token-item-quota';
    quotaEl.textContent = `余额: ${quota}`;
    
    info.appendChild(emailEl);
    info.appendChild(quotaEl);
    item.appendChild(info);
    
    if (isActive) {
      const activeLabel = document.createElement('span');
      activeLabel.style.cssText = 'color:var(--primary);font-size:11px;';
      activeLabel.textContent = '已激活';
      item.appendChild(activeLabel);
    } else {
      const activateBtn = document.createElement('button');
      activateBtn.className = 'btn btn-sm btn-primary';
      activateBtn.textContent = '激活';
      activateBtn.dataset.action = 'activate';
      activateBtn.dataset.tokenId = token.id;
      item.appendChild(activateBtn);
    }
    
    elements.tokenItems.appendChild(item);
  });
}

// 抽屉交互逻辑
function setupDrawerInteractions() {
  const { drawer, drawerBackdrop, btnOpenDrawer, drawerHandle } = elements;
  
  function openDrawer() {
    drawer.classList.add('visible');
    drawerBackdrop.classList.add('visible');
  }
  
  function closeDrawer() {
    drawer.classList.remove('visible');
    drawerBackdrop.classList.remove('visible');
  }
  
  function toggleDrawer() {
    if (drawer.classList.contains('visible')) {
      closeDrawer();
    } else {
      openDrawer();
    }
  }
  
  btnOpenDrawer.addEventListener('click', openDrawer);
  drawerBackdrop.addEventListener('click', closeDrawer);
  
  // 简单的点击把手关闭 (也可以扩展为拖拽)
  drawerHandle.addEventListener('click', toggleDrawer);
}

// 更新当前账号显示 (适配极简聚焦布局)
function updateCurrentAccount() {
  const activeToken = state.tokens.find(t => t.id === state.activeTokenId);
  
  if (activeToken) {
    elements.accountEmail.textContent = activeToken.email || '未知账号';
    
    const total = activeToken.quota_total;
    const used = activeToken.quota_used;
    const remaining = total - used;
    const percentage = Math.max(0, Math.min(100, (remaining / total) * 100));
    
    // 更新大数字
    if (elements.quotaValue) {
      elements.quotaValue.textContent = formatQuota(remaining);
    }
    
    // 更新进度条
    if (elements.quotaProgress) {
      elements.quotaProgress.style.width = `${percentage}%`;
      // 根据剩余量改变颜色
      if (percentage < 20) {
        elements.quotaProgress.style.backgroundColor = 'var(--danger)';
      } else if (percentage < 50) {
        elements.quotaProgress.style.backgroundColor = 'var(--warning)';
      } else {
        elements.quotaProgress.style.backgroundColor = 'var(--primary)';
      }
    }
    
    // 更新总额文本
    if (elements.quotaTotal) {
      elements.quotaTotal.textContent = `总额: ${formatQuota(total)}`;
    }
    
    // 隐藏提示箭头
    if (elements.activationArrow) elements.activationArrow.style.display = 'none';
    
  } else if (state.tokens.length > 0) {
    elements.accountEmail.textContent = '未选中账号';
    if (elements.quotaValue) elements.quotaValue.textContent = '--';
    if (elements.quotaProgress) elements.quotaProgress.style.width = '0%';
    if (elements.quotaTotal) elements.quotaTotal.textContent = '点击管理账户激活';
    
    // 显示提示箭头
    if (elements.activationArrow) elements.activationArrow.style.display = 'flex';
    
  } else {
    elements.accountEmail.textContent = '无可用账号';
    if (elements.quotaValue) elements.quotaValue.textContent = '--';
    if (elements.quotaProgress) elements.quotaProgress.style.width = '0%';
    if (elements.quotaTotal) elements.quotaTotal.textContent = '请添加激活码';
    
    // 显示提示箭头 (引导添加)
    if (elements.activationArrow) elements.activationArrow.style.display = 'flex';
  }
}

// 激活 Token
async function activateToken(tokenId) {
  const token = state.tokens.find(t => t.id === tokenId);
  if (!token) return;
  
  showToast('loading', '正在切换账号...');
  
  try {
    const result = await invoke('activate_token', { tokenId });
    if (result.success) {
      state.activeTokenId = tokenId;
      renderTokens();
      updateCurrentAccount();
      setStatus(`已激活: ${token.email}`);
      showToast('success', '切换成功', 1500);
      // 订阅该 token 的实时更新
      subscribeTokenUpdate(tokenId);
    } else {
      setStatus(`激活失败`);
      showToast('error', `切换失败: ${result.error}`, 3000);
    }
  } catch (e) {
    setStatus(`激活失败`);
    showToast('error', `切换失败: ${e}`, 3000);
  }
}

// 退出
async function handleLogout() {
  closeExitModal();
  try {
    await invoke('logout');
  } catch (e) {
    console.error('退出失败:', e);
  }
  
  state.isLoggedIn = false;
  state.tokens = [];
  state.activeTokenId = null;
  showLoginPage();
  setStatus('已退出');
}

// 清理数据退出
async function handleLogoutWithClear() {
  closeExitModal();
  showToast('loading', '正在清理数据...');
  
  try {
    await invoke('clear_all_data');
    // 清除本地存储
    localStorage.removeItem('noticeDismissedVersion');
    localStorage.removeItem('closeBehavior');
  } catch (e) {
    console.error('清理数据失败:', e);
  }
  
  state.isLoggedIn = false;
  state.tokens = [];
  state.activeTokenId = null;
  showLoginPage();
  showToast('success', '数据已清理', 1500);
}

// 解绑设备并退出
async function handleUnbindAndExit() {
  closeExitModal();
  showToast('loading', '正在解绑设备...');
  
  try {
    const result = await invoke('unbind_and_clear');
    // 清除本地存储
    localStorage.removeItem('noticeDismissedVersion');
    localStorage.removeItem('closeBehavior');
    
    if (result.results && result.results.length > 0) {
      console.log('解绑结果:', result.results);
    }
  } catch (e) {
    console.error('解绑失败:', e);
  }
  
  state.isLoggedIn = false;
  state.tokens = [];
  state.activeTokenId = null;
  showLoginPage();
  showToast('success', '设备已解绑，数据已清理', 2000);
}

// 页面切换
function showMainPage() {
  // 隐藏其他页面
  elements.pageLogin.style.display = 'none';
  elements.pageLogin.classList.remove('page-active');
  elements.pageLogin.classList.add('page-inactive');
  
  const pageSelect = document.getElementById('page-mode-select');
  if (pageSelect) {
    pageSelect.style.display = 'none';
    pageSelect.classList.remove('page-active');
    pageSelect.classList.add('page-inactive');
  }
  
  // 显示主页面
  elements.pageMain.style.display = '';
  elements.pageMain.classList.remove('page-inactive');
  elements.pageMain.classList.add('page-active');
  
  loadSavedCodes();
  updateModeIndicator(); // 更新模式指示器
}

function showLoginPage() {
  elements.pageMain.classList.remove('page-active');
  elements.pageMain.classList.add('page-inactive');
  
  elements.pageLogin.classList.remove('page-inactive');
  elements.pageLogin.classList.add('page-active');
  
  elements.activationCode.value = '';
  hideError();
}

// UI 辅助
function setLoading(loading, text = '正在登录中...') {
  elements.btnActivate.disabled = loading;
  elements.btnActivate.querySelector('.btn-text').style.display = loading ? 'none' : '';
  elements.btnActivate.querySelector('.btn-loading').style.display = loading ? '' : 'none';
  
  // 显示/隐藏全屏加载遮罩
  const overlay = document.getElementById('loading-overlay');
  const loadingText = document.getElementById('loading-text');
  if (overlay) {
    overlay.style.display = loading ? 'flex' : 'none';
    if (loadingText) loadingText.textContent = text;
  }
}

function showError(msg) {
  elements.loginError.textContent = msg;
  elements.loginError.style.display = 'block';
}

function hideError() {
  elements.loginError.style.display = 'none';
}

function setStatus(text) {
  elements.statusText.textContent = text;
}

// 显示/隐藏加载遮罩（独立函数，用于自动登录等场景）
function showLoadingOverlay(text = '加载中...') {
  const overlay = document.getElementById('loading-overlay');
  const loadingText = document.getElementById('loading-text');
  if (overlay) {
    overlay.style.display = 'flex';
    if (loadingText) loadingText.textContent = text;
  }
}

function hideLoadingOverlay() {
  const overlay = document.getElementById('loading-overlay');
  if (overlay) {
    overlay.style.display = 'none';
  }
}

// Toast 提示
let toastTimer = null;
function showToast(type, message, duration = 0) {
  const toast = document.getElementById('toast');
  const icon = document.getElementById('toast-icon');
  const msg = document.getElementById('toast-message');
  
  // 清除之前的定时器
  if (toastTimer) {
    clearTimeout(toastTimer);
    toastTimer = null;
  }
  
  // 设置图标
  icon.className = 'toast-icon';
  icon.textContent = '';
  if (type === 'loading') {
    icon.classList.add('loading');
  } else if (type === 'success') {
    icon.classList.add('success');
    icon.textContent = '✓';
  } else if (type === 'error') {
    icon.classList.add('error');
    icon.textContent = '✕';
  }
  
  msg.textContent = message;
  toast.classList.add('visible');
  
  // 自动隐藏
  if (duration > 0) {
    toastTimer = setTimeout(() => {
      toast.classList.remove('visible');
    }, duration);
  }
}

function hideToast() {
  const toast = document.getElementById('toast');
  toast.classList.remove('visible');
  if (toastTimer) {
    clearTimeout(toastTimer);
    toastTimer = null;
  }
}

function formatQuota(num) {
  if (!num) return '0';
  if (num >= 1000000) return (num / 1000000).toFixed(1) + 'M';
  if (num >= 1000) return (num / 1000).toFixed(1) + 'K';
  return num.toString();
}

// 安全转义：防止 XSS 注入（包含 ' " 转义，防止属性/JS字符串上下文注入）
function escapeHtml(str) {
  if (!str) return '';
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#x27;');
}

// 心跳
let heartbeatTimer = null;
function startHeartbeat() {
  if (heartbeatTimer) return;
  heartbeatTimer = setInterval(async () => {
    if (!state.isLoggedIn) return;
    try {
      const result = await invoke('heartbeat');
      if (!result.valid) {
        state.isLoggedIn = false;
        showLoginPage();
        showError('会话已过期');
      }
    } catch (e) {}
  }, 60000);
}

// 自动刷新 token 列表
let autoRefreshTimer = null;
function startAutoRefresh() {
  if (autoRefreshTimer) return;
  autoRefreshTimer = setInterval(async () => {
    if (!state.isLoggedIn || state.tokens.length === 0) return;
    await loadTokens();
  }, 30000);
}

// ==================== WebSocket 实时同步 ====================
let ws = null;
let wsReconnectTimer = null;
let wsHeartbeatTimer = null;
const WS_URL = 'wss://dd.776523718.xyz/ws'; // WebSocket 服务器地址

function connectWebSocket() {
  if (ws && ws.readyState === WebSocket.OPEN) return;
  
  try {
    ws = new WebSocket(WS_URL);
    
    ws.onopen = () => {
      console.log('[WebSocket] 已连接');
      // 如果有激活的 token，订阅更新
      if (state.activeTokenId) {
        subscribeTokenUpdate(state.activeTokenId);
      }
    };
    
    ws.onmessage = async (event) => {
      try {
        const msg = JSON.parse(event.data);
        console.log('[WebSocket] 收到消息:', msg);
        
        if (msg.type === 'token_updated' && msg.token_id === state.activeTokenId) {
          console.log('[WebSocket] Token 已更新，立即同步...');
          // 立即同步本地 token
          try {
            const result = await invoke('refresh_active_token', { force: true });
            if (result.refreshed) {
              console.log('[WebSocket] 本地 token 已同步');
              showToast('success', 'Token 已自动同步', 2000);
            }
          } catch (e) {
            console.error('[WebSocket] 同步失败:', e);
          }
        }
        
        if (msg.type === 'pong') {
          console.log('[WebSocket] 心跳正常');
        }
      } catch (e) {
        console.error('[WebSocket] 消息解析错误:', e);
      }
    };
    
    ws.onclose = () => {
      console.log('[WebSocket] 连接关闭，5秒后重连...');
      ws = null;
      // 5秒后重连
      if (wsReconnectTimer) clearTimeout(wsReconnectTimer);
      wsReconnectTimer = setTimeout(connectWebSocket, 5000);
    };
    
    ws.onerror = (err) => {
      console.error('[WebSocket] 错误:', err);
    };
  } catch (e) {
    console.error('[WebSocket] 连接失败:', e);
  }
}

// 订阅特定 token 的更新
function subscribeTokenUpdate(tokenId) {
  if (ws && ws.readyState === WebSocket.OPEN && tokenId) {
    ws.send(JSON.stringify({ type: 'subscribe', token_id: tokenId }));
    console.log('[WebSocket] 订阅 token:', tokenId);
  }
}

// WebSocket 心跳
function startWsHeartbeat() {
  if (wsHeartbeatTimer) return; // 防止重复创建
  wsHeartbeatTimer = setInterval(() => {
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: 'ping' }));
    }
  }, 30000);
}

// ==================== 自动更新 ====================
let updateInfo = null;

async function checkForUpdate() {
  console.log('[Update] 开始检查更新...');
  
  // 显示检查中状态
  const btn = document.getElementById('btn-check-update');
  if (btn) {
    btn.disabled = true;
    btn.textContent = '检查中...';
  }
  
  try {
    const result = await invoke('check_update');
    console.log('[Update] 检查结果:', result);
    if (result.hasUpdate) {
      console.log('[Update] 发现新版本!');
      updateInfo = result;
      showUpdateModal(result);
    } else {
      console.log('[Update] 已是最新版本');
      showToast('success', '已是最新版本 ✓', 2000);
    }
  } catch (e) {
    console.error('[Update] 检查更新失败:', e);
    showToast('error', '检查更新失败: ' + e, 3000);
  } finally {
    // 恢复按钮状态
    if (btn) {
      btn.disabled = false;
      btn.textContent = '检查更新';
    }
  }
}

// 检测是否为 macOS
function isMacOS() {
  return navigator.platform.toUpperCase().indexOf('MAC') >= 0;
}

function showUpdateModal(info) {
  document.getElementById('update-version').textContent = 'v' + info.version;
  document.getElementById('update-size').textContent = formatSize(info.size || 0);
  document.getElementById('update-changelog').textContent = info.changelog || '修复已知问题，提升稳定性';
  document.getElementById('modal-update').style.display = 'flex';
  
  const btnUpdate = document.getElementById('btn-do-update');
  
  // macOS 显示"打开下载页面"按钮
  if (isMacOS()) {
    btnUpdate.textContent = '打开下载页面';
  } else {
    btnUpdate.textContent = '立即更新';
  }
  
  // 强制更新时显示退出按钮，隐藏跳过按钮
  if (info.forceUpdate) {
    document.getElementById('btn-skip-update').textContent = '退出软件';
    document.getElementById('btn-skip-update').style.display = '';
    document.getElementById('btn-close-update').style.display = 'none';
  } else {
    document.getElementById('btn-skip-update').textContent = '稍后再说';
    document.getElementById('btn-skip-update').style.display = '';
    document.getElementById('btn-close-update').style.display = '';
  }
}

function formatSize(bytes) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
}

let updateUnlisten = null;

async function doUpdate() {
  if (!updateInfo || !updateInfo.downloadUrl) {
    showToast('error', '更新信息无效');
    return;
  }
  
  // macOS: 直接打开浏览器下载页面
  if (isMacOS()) {
    try {
      await invoke('open_download_url', { downloadUrl: updateInfo.downloadUrl });
      showToast('success', '已打开下载页面，请手动安装新版本', 3000);
      closeUpdateModal();
    } catch (e) {
      showToast('error', '打开下载页面失败: ' + e, 3000);
    }
    return;
  }
  
  // Windows: 自动下载更新
  const btnUpdate = document.getElementById('btn-do-update');
  const btnSkip = document.getElementById('btn-skip-update');
  const btnClose = document.getElementById('btn-close-update');
  const progressContainer = document.getElementById('update-progress');
  const progressBar = document.getElementById('update-progress-bar');
  const progressText = document.getElementById('update-progress-text');
  
  // 禁用按钮，显示进度
  btnUpdate.disabled = true;
  btnUpdate.textContent = '下载中...';
  btnSkip.style.display = 'none';
  btnClose.style.display = 'none';
  progressContainer.style.display = 'block';
  progressBar.style.width = '0%';
  progressText.textContent = '准备下载...';
  
  // 监听下载进度
  try {
    updateUnlisten = await listen('update-progress', (event) => {
      const { downloaded, total, percent } = event.payload;
      progressBar.style.width = `${percent}%`;
      if (total > 0) {
        progressText.textContent = `${formatSize(downloaded)} / ${formatSize(total)} (${percent}%)`;
      } else {
        progressText.textContent = `已下载 ${formatSize(downloaded)}`;
      }
    });
  } catch (e) {
    console.error('监听进度事件失败:', e);
  }
  
  // 执行下载和更新
  try {
    progressText.textContent = '正在下载...';
    await invoke('download_and_update', { downloadUrl: updateInfo.downloadUrl });
    // 如果成功，程序会自动重启，不会执行到这里
  } catch (e) {
    console.error('[Update] 更新失败:', e);
    showToast('error', '更新失败: ' + e, 5000);
    
    // 恢复按钮状态
    btnUpdate.disabled = false;
    btnUpdate.textContent = '立即更新';
    btnSkip.style.display = '';
    if (!updateInfo.forceUpdate) {
      btnClose.style.display = '';
    }
    progressContainer.style.display = 'none';
  } finally {
    // 清理监听器
    if (updateUnlisten) {
      updateUnlisten();
      updateUnlisten = null;
    }
  }
}

async function closeUpdateModal() {
  // 强制更新时点击退出软件
  if (updateInfo && updateInfo.forceUpdate) {
    await invoke('exit_app');
    return;
  }
  document.getElementById('modal-update').style.display = 'none';
}

// 绑定更新弹窗事件
document.getElementById('btn-do-update')?.addEventListener('click', doUpdate);
document.getElementById('btn-skip-update')?.addEventListener('click', closeUpdateModal);
document.getElementById('btn-close-update')?.addEventListener('click', closeUpdateModal);
document.getElementById('btn-check-update')?.addEventListener('click', checkForUpdate);

// 启动
document.addEventListener('DOMContentLoaded', () => {
  init();
  // 延迟检查更新
  setTimeout(checkForUpdate, 3000);
  // 每30分钟检查一次更新
  setInterval(checkForUpdate, 30 * 60 * 1000);
  
  // 定期检查页面可见性（防止黑屏）
  setInterval(ensurePageVisible, 5000);
  
  // 绑定模式选择按钮事件
  document.getElementById('btn-mode-normal')?.addEventListener('click', () => {
    console.log('[ModeSelect] 点击正常模式');
    selectMode('normal');
  });
  document.getElementById('btn-mode-autoswitch')?.addEventListener('click', () => {
    console.log('[ModeSelect] 点击自动切换模式');
    selectMode('autoswitch');
  });
});
