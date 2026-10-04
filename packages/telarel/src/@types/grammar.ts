/**
 * The grammar of the source code.
 *
 * - `js` — JavaScript
 * - `ts` — TypeScript
 * - `dts` — TypeScript declaration
 * - `jsx` — JavaScript with JSX
 * - `tsx` — TypeScript with JSX
 */
type Language = "js" | "ts" | "dts" | "jsx" | "tsx";

/**
 * The module system / execution mode of the source code.
 *
 * - `script` — classic non-module script
 * - `commonjs` — CommonJS (`require` / `module.exports`)
 * - `module` — ES Module (`import` / `export`)
 * - `unambiguous` - the parser infers from the statements
 */
type SourceType = "script" | "commonjs" | "module" | "unambiguous";

export type { Language, SourceType };
