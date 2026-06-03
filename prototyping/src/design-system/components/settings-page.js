import './settings-page.css';
import { el } from '../utils/dom.js';

/**
 * @param {object} options
 * @param {Array<{ label: string, key: string, items: Array<{ title: string, description: string, control: Element }> }>} options.sections
 */
export function createSettingsPage({ sections = [] } = {}) {
  const navItems = [];
  const paneContainers = [];

  sections.forEach((sec, idx) => {
    const isFirst = idx === 0;

    // Navigation Item
    const navItem = el('div', {
      class: `ff-settings-page__nav-item ${isFirst ? 'ff-settings-page__nav-item--active' : ''}`,
      dataset: { tabKey: sec.key || String(idx) },
      onClick: (e) => {
        // Switch tab active states
        const allNavs = element.querySelectorAll('.ff-settings-page__nav-item');
        allNavs.forEach(nav => nav.classList.remove('ff-settings-page__nav-item--active'));
        e.currentTarget.classList.add('ff-settings-page__nav-item--active');

        // Switch pane visibility
        const allPanes = element.querySelectorAll('.ff-settings-page__pane-tab-content');
        allPanes.forEach(pane => pane.classList.remove('ff-settings-page__pane-tab-content--active'));
        const activePane = element.querySelector(`[data-pane-key="${sec.key || idx}"]`);
        if (activePane) {
          activePane.classList.add('ff-settings-page__pane-tab-content--active');
        }
      }
    }, [sec.label]);

    navItems.push(navItem);

    // Pane Content for Section
    const fields = sec.items.map(field => {
      const labelText = el('span', { class: 'ff-settings-page__field-label' }, [field.title]);
      const descText = el('span', { class: 'ff-settings-page__field-desc' }, [field.description]);

      const textCol = el('div', { class: 'ff-settings-page__field-texts' }, [
        labelText,
        descText
      ]);

      const controlContainer = el('div', { class: 'ff-settings-page__field-control' }, [
        field.control
      ]);

      return el('div', { class: 'ff-settings-page__field' }, [
        textCol,
        controlContainer
      ]);
    });

    const paneContent = el('div', {
      class: `ff-settings-page__pane-tab-content ${isFirst ? 'ff-settings-page__pane-tab-content--active' : ''}`,
      dataset: { paneKey: sec.key || String(idx) }
    }, [
      el('h3', { class: 'ff-settings-page__pane-heading' }, [sec.label]),
      el('p', { class: 'ff-settings-page__pane-sub' }, [`Configure settings for ${sec.label}`]),
      el('div', { class: 'ff-settings-page__fields-container' }, fields)
    ]);

    paneContainers.push(paneContent);
  });

  const nav = el('div', { class: 'ff-settings-page__nav' }, [
    el('div', { class: 'ff-settings-page__nav-head' }, ['Preferences']),
    ...navItems
  ]);

  const pane = el('div', { class: 'ff-settings-page__pane' }, paneContainers);

  const element = el('div', { class: 'ff-settings-page' }, [
    nav,
    pane
  ]);

  return { element };
}
