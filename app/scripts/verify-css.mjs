#!/usr/bin/env node
// Guards against a silent Tailwind mis-wire: `vite build` exits 0 even when the
// `@tailwindcss/vite` plugin isn't registered — in that case `@import "tailwindcss"`
// and `@theme{}` ship LITERALLY into dist CSS and the app renders unstyled.
// This asserts the v4 compile actually ran: real utilities present, no literal
// at-rules leaked. Run after `build`.
import { readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'

const dir = 'dist/assets'
let files
try {
  files = readdirSync(dir).filter((f) => f.endsWith('.css'))
} catch {
  console.error(`verify-css: no ${dir} — run \`vite build\` first`)
  process.exit(1)
}
if (files.length === 0) {
  console.error('verify-css: no compiled CSS emitted')
  process.exit(1)
}

const css = files.map((f) => readFileSync(join(dir, f), 'utf8')).join('\n')
const problems = []

// Literal directives that must have been consumed by the v4 compile pass.
if (/@tailwind\s+utilities/.test(css)) problems.push('literal `@tailwind utilities` leaked (plugin not wired?)')
if (/@theme\s*\{/.test(css)) problems.push('literal `@theme{}` leaked (not lowered to :root)')

// Real evidence the compile ran: theme var lowered + a couple of token utilities.
if (!css.includes('--color-parchment')) problems.push('theme var `--color-parchment` not emitted')
for (const util of ['.bg-chestnut', '.shadow-bloom', '.font-serif']) {
  if (!css.includes(util)) problems.push(`utility \`${util}\` not generated`)
}

if (problems.length) {
  console.error('verify-css FAILED:\n  - ' + problems.join('\n  - '))
  process.exit(1)
}
console.log('verify-css OK: Tailwind v4 compiled (utilities + lowered theme vars present)')
