export function flagValues(argv: readonly string[], name: string): string[] {
  const out: string[] = [];
  argv.forEach((arg, i) => {
    if (arg === `--${name}`) {
      const value = argv[i + 1];
      if (value === undefined || value.startsWith('--')) throw new Error(`--${name} needs a value`);
      out.push(value);
    } else if (arg.startsWith(`--${name}=`)) out.push(arg.slice(name.length + 3));
  });
  return out;
}

export function flagValue(argv: readonly string[], name: string): string | undefined {
  const values = flagValues(argv, name);
  if (values.length > 1) throw new Error(`--${name} given more than once`);
  return values[0];
}
