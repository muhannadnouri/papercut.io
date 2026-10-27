import js from '@eslint/js'
import stylistic from '@stylistic/eslint-plugin'
import importNewlines from 'eslint-plugin-import-newlines'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import tseslint from 'typescript-eslint'
import {
  defineConfig,
  globalIgnores,
} from 'eslint/config'

export default defineConfig([
  globalIgnores([
    'dist',
    '.venv-silma/**',
    'src-tauri/target/**',
    'src-tauri/gen/**',
    'public/pdfjs/**',
    'public/tesseract/**',
  ]),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2020,
      globals: globals.browser,
    },
    rules: {
      // Visibility comes from TypeScript; do not guess privacy from a module-local function's name.
      '@typescript-eslint/naming-convention': [
        'error',
        {
          selector: 'classMethod',
          modifiers: ['private'],
          format: ['camelCase'],
          leadingUnderscore: 'require',
        },
        {
          selector: 'classMethod',
          modifiers: ['#private'],
          format: null,
          leadingUnderscore: 'forbid',
        },
        {
          selector: 'classMethod',
          format: null,
          leadingUnderscore: 'forbid',
        },
      ],
    },
  },
  {
    files: ['**/*.{js,mjs,ts,tsx}'],
    plugins: {
      '@stylistic': stylistic,
      'import-newlines': importNewlines,
    },
    // shortcut: warn on legacy layout until a separate formatting-only cleanup; lint:strict enforces zero warnings.
    rules: {
      'curly': ['warn', 'all'],
      'one-var': ['warn', 'never'],
      '@stylistic/max-statements-per-line': ['warn', { max: 1 }],
      '@stylistic/brace-style': ['warn', '1tbs'],
      '@stylistic/object-curly-newline': ['warn', {
        ObjectExpression: {
          multiline: true,
          minProperties: 2,
          consistent: true,
        },
      }],
      '@stylistic/object-property-newline': 'warn',
      '@stylistic/indent': ['warn', 2],
      '@stylistic/no-trailing-spaces': 'warn',
      '@stylistic/eol-last': ['warn', 'always'],
      '@stylistic/no-multiple-empty-lines': ['warn', {
        max: 1,
        maxEOF: 0,
        maxBOF: 0,
      }],
      '@stylistic/comma-dangle': ['warn', 'always-multiline'],
      'import-newlines/enforce': ['warn', {
        items: 1,
        forceSingleLine: false,
        semi: false,
      }],
    },
  },
])
