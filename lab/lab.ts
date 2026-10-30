import { mountInbox, resolveApp, type LabConfig } from 'labkit';
import config from './labkit.config.json';

mountInbox(document.getElementById('lab')!, { app: resolveApp(config as LabConfig) });
