import type { FullResult, Reporter, TestCase } from '@playwright/test/reporter';
import { loadavg } from 'node:os';

export default class SmokeReporter implements Reporter {
  private failed: string[] = [];
  private flaky: string[] = [];

  onTestEnd(test: TestCase) {
    const outcome = test.outcome();
    const name = `${test.location.file.split('/smoke/').pop()}:${test.location.line} ${test.title}`;
    if (outcome === 'unexpected') this.failed.push(name);
    if (outcome === 'flaky') this.flaky.push(name);
  }

  onEnd(result: FullResult) {
    const lines = [`smoke: status ${result.status}, ${this.failed.length} failed, ${this.flaky.length} flaky, load average ${loadavg()[0].toFixed(1)}`];
    for (const name of this.failed) lines.push(`FAILED ${name}`);
    for (const name of this.flaky) lines.push(`FLAKY ${name}`);
    console.log(`\n${lines.join('\n')}`);
  }

  printsToStdio() {
    return false;
  }
}
