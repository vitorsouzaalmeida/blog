import ts from "typescript-eslint";
import astro from "eslint-plugin-astro";
import prettier from "eslint-config-prettier";

export default [
  ...ts.configs.recommended,
  ...astro.configs.recommended,
  prettier, // disables ESLint rules that conflict with Prettier — keep last
  { ignores: ["dist/", ".astro/", "node_modules/", "src/env.d.ts"] },
];
