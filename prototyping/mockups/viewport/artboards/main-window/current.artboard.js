import { host, MainWindow } from '#design-system/index.js';

export const meta = { title: 'Main window: current', layer: 'composite', group: 'Main window', order: 0 };

export default function render() {
  return host(MainWindow);
}
