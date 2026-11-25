// Panel JS must run on the stock Android 7 WebView (Chrome 51): eslint-plugin-compat checks
// browser APIs against package.json "browserslist"; syntax is lowered by esbuild --target=chrome51.
import js from '@eslint/js';
import compat from 'eslint-plugin-compat';
import globals from 'globals';

export default [
  js.configs.recommended,
  {
    files: ['src/**/*.js'],
    plugins: { compat },
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: { ...globals.browser },
    },
    settings: {
      // Promise, fetch and JSON exist in Chrome 51; nothing is polyfilled.
      polyfills: [],
    },
    rules: {
      'compat/compat': 'error',
      'no-unused-vars': ['error', { args: 'none', caughtErrors: 'none' }],
    },
  },
  {
    files: ['src/demo.js'],
    // shared with dev-server.mjs (Node) — keep it free of browser-only APIs
    languageOptions: { globals: { ...globals.browser, ...globals.node } },
  },
  {
    files: ['*.mjs'],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: { ...globals.node },
    },
  },
];
