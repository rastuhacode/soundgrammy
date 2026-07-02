import js from "@eslint/js";
import stylistic from "@stylistic/eslint-plugin";
import { importX } from "eslint-plugin-import-x";
import pluginReact from "eslint-plugin-react";
import pluginReactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

/** @type {import("eslint").Linter.Config[]} */
export default [
  js.configs.recommended,
  ...tseslint.configs.recommended,
  importX.flatConfigs.recommended,
  importX.flatConfigs.typescript,
  stylistic.configs.recommended,
  {
    settings: {
      "import-x/resolver": {
        typescript: {
          project: "./tsconfig.json",
        },
      },
    },
  },
  // eslint-plugin-react@7 is not compatible with ESLint 10 yet: with version "detect"
  // it calls context.getFilename(), which was removed in ESLint 10 and crashes lint.
  // Track: https://github.com/jsx-eslint/eslint-plugin-react/issues/3977
  // Remove the pinned version below and switch back to "detect" once upstream adds support.
  pluginReact.configs.flat.recommended,
  {
    languageOptions: {
      ...pluginReact.configs.flat.recommended.languageOptions,
      globals: {
        ...globals.browser,
      },
    },
    settings: { react: { version: "19.2" } },
  },
  {
    plugins: {
      "react-hooks": pluginReactHooks,
    },
    rules: {
      ...pluginReactHooks.configs.recommended.rules,
      "react/react-in-jsx-scope": "off",
    },
  },
  {
    // Same eslint-plugin-react + ESLint 10 crash when this file is linted with react rules.
    ignores: ["dist/**", "src-tauri/target/**", "eslint.config.js"],
  },
];
