const path = require("path");
const CopyPlugin = require("copy-webpack-plugin");
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");
const webpack = require("webpack");

const dist = path.resolve(__dirname, "dist");

module.exports = {
  mode: "development",
  experiments: {
    asyncWebAssembly: true,
  },
  module: {
    rules: [{
      test: /\.wasm$/,
      type: "webassembly/async"
    }]
  },
  entry: {
    index: "./static/index.js"
  },
  output: {
    hashFunction: "xxhash64",
    path: dist,
    filename: "[name].js"
  },
  devServer: {
    static: [dist],
  },
  performance: { hints: false },
  plugins: [
    new CopyPlugin({
      patterns: [
        path.resolve(__dirname, "static")
      ]
    }),

    new WasmPackPlugin({
      crateDirectory: __dirname,
    }),
    new webpack.LoaderOptionsPlugin({
      options: {
        experiments: {
          asyncWebAssembly: true
        }
      }
    }),
  ]
};
