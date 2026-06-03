let activeMenu = null;
let activeDialog = null;

function removeActiveMenu() {
  if (!activeMenu) return;
  activeMenu.remove();
  activeMenu = null;
}

function removeActiveDialog() {
  if (!activeDialog) return;
  activeDialog.remove();
  activeDialog = null;
}

function closeAllOverlays() {
  removeActiveMenu();
  removeActiveDialog();
}

function createBaseDialog(title) {
  removeActiveDialog();

  const overlay = document.createElement('div');
  overlay.className = 'sidebar-dialog-overlay';

  const dialog = document.createElement('div');
  dialog.className = 'sidebar-dialog';

  const titleEl = document.createElement('div');
  titleEl.className = 'sidebar-dialog-title';
  titleEl.textContent = title;
  dialog.appendChild(titleEl);

  overlay.appendChild(dialog);
  document.body.appendChild(overlay);
  activeDialog = overlay;
  return { overlay, dialog };
}

function createMenuButton(item) {
  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'sidebar-context-menu-item';
  if (item.muted) button.classList.add('sidebar-context-menu-item--muted');
  button.textContent = item.label;
  button.addEventListener('click', (event) => {
    event.stopPropagation();
    removeActiveMenu();
    item.onSelect?.();
  });
  return button;
}

function positionMenu(menu, x, y) {
  menu.style.left = `${x}px`;
  menu.style.top = `${y}px`;
  document.body.appendChild(menu);
  const rect = menu.getBoundingClientRect();
  const left = Math.max(8, Math.min(x, window.innerWidth - rect.width - 8));
  const top = Math.max(8, Math.min(y, window.innerHeight - rect.height - 8));
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;
}

export function showContextMenu(event, items) {
  event.preventDefault();
  event.stopPropagation();
  removeActiveMenu();

  const menu = document.createElement('div');
  menu.className = 'sidebar-context-menu';

  items.forEach((item) => {
    if (item.type === 'divider') {
      const divider = document.createElement('div');
      divider.className = 'sidebar-context-menu-divider';
      menu.appendChild(divider);
      return;
    }
    if (item.type === 'label') {
      const label = document.createElement('div');
      label.className = 'sidebar-context-menu-label';
      label.textContent = item.label;
      menu.appendChild(label);
      return;
    }
    menu.appendChild(createMenuButton(item));
  });

  positionMenu(menu, event.clientX, event.clientY);
  activeMenu = menu;
}

export function showRenameDialog(currentName, onSubmit) {
  const { overlay, dialog } = createBaseDialog('Edit name');

  const input = document.createElement('input');
  input.className = 'sidebar-dialog-input';
  input.type = 'text';
  input.value = currentName || '';
  dialog.appendChild(input);

  const actions = document.createElement('div');
  actions.className = 'sidebar-dialog-actions';

  const cancelBtn = document.createElement('button');
  cancelBtn.type = 'button';
  cancelBtn.className = 'sidebar-dialog-btn sidebar-dialog-btn--ghost';
  cancelBtn.textContent = 'Cancel';
  actions.appendChild(cancelBtn);

  const saveBtn = document.createElement('button');
  saveBtn.type = 'button';
  saveBtn.className = 'sidebar-dialog-btn sidebar-dialog-btn--primary';
  saveBtn.textContent = 'Save';
  actions.appendChild(saveBtn);
  dialog.appendChild(actions);

  const close = () => removeActiveDialog();
  const submit = () => {
    const value = input.value.trim();
    if (value) onSubmit?.(value);
    close();
  };

  cancelBtn.addEventListener('click', close);
  saveBtn.addEventListener('click', submit);
  overlay.addEventListener('click', (event) => {
    if (event.target === overlay) close();
  });
  input.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') submit();
    if (event.key === 'Escape') close();
  });

  requestAnimationFrame(() => {
    input.focus();
    input.select();
  });
}

export function showTextDialog({ title, initialValue = '', placeholder = '', confirmLabel = 'Save', onSubmit }) {
  const { overlay, dialog } = createBaseDialog(title);

  const input = document.createElement('textarea');
  input.className = 'sidebar-dialog-input sidebar-dialog-input--multiline';
  input.value = initialValue;
  input.placeholder = placeholder;
  dialog.appendChild(input);

  const actions = document.createElement('div');
  actions.className = 'sidebar-dialog-actions';

  const cancelBtn = document.createElement('button');
  cancelBtn.type = 'button';
  cancelBtn.className = 'sidebar-dialog-btn sidebar-dialog-btn--ghost';
  cancelBtn.textContent = 'Cancel';
  actions.appendChild(cancelBtn);

  const saveBtn = document.createElement('button');
  saveBtn.type = 'button';
  saveBtn.className = 'sidebar-dialog-btn sidebar-dialog-btn--primary';
  saveBtn.textContent = confirmLabel;
  actions.appendChild(saveBtn);
  dialog.appendChild(actions);

  const close = () => removeActiveDialog();
  const submit = () => {
    onSubmit?.(input.value);
    close();
  };

  cancelBtn.addEventListener('click', close);
  saveBtn.addEventListener('click', submit);
  overlay.addEventListener('click', (event) => {
    if (event.target === overlay) close();
  });
  input.addEventListener('keydown', (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') submit();
    if (event.key === 'Escape') close();
  });

  requestAnimationFrame(() => {
    input.focus();
  });
}

export function showProjectPickerDialog({ projects, currentProjectId, onSelect }) {
  const { overlay, dialog } = createBaseDialog('Move to project');

  const select = document.createElement('select');
  select.className = 'sidebar-dialog-select';
  projects.forEach((project) => {
    const option = document.createElement('option');
    option.value = project.id;
    option.textContent = project.name;
    if (project.id === currentProjectId) option.selected = true;
    select.appendChild(option);
  });
  dialog.appendChild(select);

  const actions = document.createElement('div');
  actions.className = 'sidebar-dialog-actions';

  const cancelBtn = document.createElement('button');
  cancelBtn.type = 'button';
  cancelBtn.className = 'sidebar-dialog-btn sidebar-dialog-btn--ghost';
  cancelBtn.textContent = 'Cancel';
  actions.appendChild(cancelBtn);

  const moveBtn = document.createElement('button');
  moveBtn.type = 'button';
  moveBtn.className = 'sidebar-dialog-btn sidebar-dialog-btn--primary';
  moveBtn.textContent = 'Move';
  actions.appendChild(moveBtn);
  dialog.appendChild(actions);

  const close = () => removeActiveDialog();
  const submit = () => {
    onSelect?.(select.value);
    close();
  };

  cancelBtn.addEventListener('click', close);
  moveBtn.addEventListener('click', submit);
  overlay.addEventListener('click', (event) => {
    if (event.target === overlay) close();
  });
}

export function showConfirmDialog({ title, message, confirmLabel = 'Confirm', onConfirm }) {
  const { overlay, dialog } = createBaseDialog(title);

  const messageEl = document.createElement('div');
  messageEl.className = 'sidebar-dialog-message';
  messageEl.textContent = message;
  dialog.appendChild(messageEl);

  const actions = document.createElement('div');
  actions.className = 'sidebar-dialog-actions';

  const cancelBtn = document.createElement('button');
  cancelBtn.type = 'button';
  cancelBtn.className = 'sidebar-dialog-btn sidebar-dialog-btn--ghost';
  cancelBtn.textContent = 'Cancel';
  actions.appendChild(cancelBtn);

  const confirmBtn = document.createElement('button');
  confirmBtn.type = 'button';
  confirmBtn.className = 'sidebar-dialog-btn sidebar-dialog-btn--primary';
  confirmBtn.textContent = confirmLabel;
  actions.appendChild(confirmBtn);
  dialog.appendChild(actions);

  const close = () => removeActiveDialog();
  cancelBtn.addEventListener('click', close);
  confirmBtn.addEventListener('click', () => {
    onConfirm?.();
    close();
  });
  overlay.addEventListener('click', (event) => {
    if (event.target === overlay) close();
  });
}

document.addEventListener('mousedown', (event) => {
  if (activeMenu && !event.target.closest('.sidebar-context-menu')) {
    removeActiveMenu();
  }
});

document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') {
    closeAllOverlays();
  }
});
