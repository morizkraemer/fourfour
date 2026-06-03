export function createProjectFrame(project, { onToggleCollapse } = {}) {
  const frameEl = document.createElement('section');
  frameEl.className = 'project-frame';
  frameEl.dataset.projectId = project.id;

  const headerEl = document.createElement('header');
  headerEl.className = 'project-frame-header';

  const titleEl = document.createElement('div');
  titleEl.className = 'project-frame-title';
  titleEl.textContent = project.name;
  headerEl.appendChild(titleEl);

  const actionsEl = document.createElement('div');
  actionsEl.className = 'project-frame-actions';

  const collapseBtn = document.createElement('button');
  collapseBtn.type = 'button';
  collapseBtn.className = 'project-frame-collapse';
  actionsEl.appendChild(collapseBtn);

  headerEl.appendChild(actionsEl);
  frameEl.appendChild(headerEl);

  const bodyEl = document.createElement('div');
  bodyEl.className = 'project-frame-body';
  frameEl.appendChild(bodyEl);

  function applyCollapsed(collapsed) {
    frameEl.classList.toggle('project-frame--collapsed', collapsed);
    collapseBtn.textContent = collapsed ? '[+]' : '[-]';
    collapseBtn.setAttribute('aria-label', collapsed ? `Expand ${project.name}` : `Collapse ${project.name}`);
  }

  collapseBtn.addEventListener('click', (event) => {
    event.stopPropagation();
    onToggleCollapse?.();
  });

  applyCollapsed(Boolean(project.collapsed));

  return {
    element: frameEl,
    bodyElement: bodyEl,
    headerElement: headerEl,
    setCollapsed: applyCollapsed,
    setTitle(title) {
      titleEl.textContent = title;
    },
  };
}
