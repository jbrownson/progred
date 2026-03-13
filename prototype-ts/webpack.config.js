let webpack = require('webpack')

module.exports = [
  {
    entry: {generator: './src/graph/generator.ts'},
    resolve: {extensions: ['.ts', '.tsx', '.js']},
    devtool: 'source-map',
    module: {
      loaders: [
        {test: /\.tsx?$/, loader: 'tslint-loader'},
        {test: /\.progred$/, loader: 'json-loader'},
        {test: /\.tsx?$/, loader: 'ts-loader'} ]},
    target: 'node',
    output: {
      path: `${__dirname}/build`,
      devtoolModuleFilenameTemplate: '[absolute-resource-path]',
      devtoolFallbackModuleFilenameTemplate: '[absolute-resource-path]?[hash]',
      filename: 'generator.js' }},
  {
    entry: { main: './src/electron/main.ts' },
    resolve: {extensions: ['.ts', '.tsx', '.js']},
    devtool: 'source-map',
    module: {
      loaders: [
        {test: /\.tsx?$/, loader: 'tslint-loader'},
        {test: /\.progred$/, loader: 'json-loader'},
        {test: /\.tsx?$/, loader: 'ts-loader'} ]},
    target: 'electron',
    node: { __dirname: false },
    output: {
      path: `${__dirname}/build`,
      devtoolModuleFilenameTemplate: '[absolute-resource-path]',
      devtoolFallbackModuleFilenameTemplate: '[absolute-resource-path]?[hash]',
      filename: 'main.js' }},
  {
    entry: { main: './src/graph/graphEditor.tsx' },
    target: 'electron',
    resolve: {extensions: ['.ts', '.tsx', '.js']},
    devtool: 'source-map',
    plugins: [new webpack.DefinePlugin({ 'process.env.NODE_ENV': '"production"' })],
    module: {
      loaders: [
        {test: /\.tsx?$/, loader: 'tslint-loader'},
        {test: /\.progred$/, loader: 'json-loader'},
        {test: /\.tsx?$/, loader: 'ts-loader'} ]},
    output: {
      path: `${__dirname}/build`,
      devtoolModuleFilenameTemplate: '[absolute-resource-path]',
      devtoolFallbackModuleFilenameTemplate: '[absolute-resource-path]?[hash]',
      filename: 'grapheditor3.js' }}]