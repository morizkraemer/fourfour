import type { Component } from 'svelte';
import { preloadAfterFirstScreen } from './afterFirstScreen.ts';

export interface LazyView {
  readonly component: Component<any> | null;
  load(): Promise<void>;
}

export function lazyView(importView: () => Promise<{ default: Component<any> }>): LazyView {
  let component = $state.raw<Component<any> | null>(null);
  let loading: Promise<void> | null = null;
  const view: LazyView = {
    get component() {
      return component;
    },
    load() {
      loading ??= importView()
        .then((module) => {
          component = module.default;
        })
        .catch((err) => {
          loading = null;
          throw err;
        });
      return loading;
    },
  };
  preloadAfterFirstScreen(() => view.load());
  return view;
}
