module.exports = {
  root: true,
  // 构建产物不参与 lint
  ignorePatterns: ['dist/', 'node_modules/'],
  env: {
    node: true,
    browser: true,
    es2021: true,
  },
  extends: [
    'plugin:vue/vue3-essential',
    'eslint:recommended',
  ],
  // .vue 文件须经 vue-eslint-parser 解析，script 块再交 TS parser
  parser: 'vue-eslint-parser',
  parserOptions: {
    ecmaVersion: 'latest',
    sourceType: 'module',
    parser: '@typescript-eslint/parser',
  },
  rules: {
    'vue/multi-word-component-names': 'off',
  },
  overrides: [
    {
      // TS 规则只作用于 .ts 源码：Vue SFC 中仅模板使用的变量会被
      // @typescript-eslint/no-unused-vars 误报，故不扩展到 .vue。
      files: ['*.ts'],
      parser: '@typescript-eslint/parser',
      parserOptions: {
        ecmaVersion: 'latest',
        sourceType: 'module',
      },
      extends: ['plugin:@typescript-eslint/recommended'],
      rules: {
        '@typescript-eslint/no-explicit-any': 'off',
        '@typescript-eslint/ban-types': 'off',
      },
    },
  ],
}
