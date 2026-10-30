import type { Page } from '@playwright/test';

export interface BootOptions {
  viewport?: { width: number; height: number };
  storage?: Record<string, string>;
  holdLoad?: boolean;
  video?: boolean;
  state?: (fixture: any) => any;
}

export interface ShotOptions {
  clip?: { x: number; y: number; width: number; height: number };
}

export interface ScenarioContext {
  boot(options?: BootOptions): Promise<Page>;
  shot(label: string, options?: ShotOptions): Promise<void>;
  finishVideo(label: string, options?: { gif?: boolean }): Promise<void>;
  pageOrThrow(): Page;
  emit(event: string, payload: unknown): Promise<void>;
}

export type Scenario = (ctx: ScenarioContext) => Promise<void>;
