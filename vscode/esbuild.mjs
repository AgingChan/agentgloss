import { build, context } from "esbuild";

const watch = process.argv.includes("--watch");

const common = {
  bundle: true,
  sourcemap: true,
  minify: !watch,
  logLevel: "info",
};

const extension = {
  ...common,
  entryPoints: ["src/extension.ts"],
  outfile: "dist/extension.js",
  platform: "node",
  format: "cjs",
  target: "node20",
  external: ["vscode"],
};

const webview = {
  ...common,
  entryPoints: ["src/webview/main.ts"],
  outfile: "dist/webview.js",
  platform: "browser",
  format: "iife",
  target: "es2022",
};

if (watch) {
  const contexts = await Promise.all([context(extension), context(webview)]);
  await Promise.all(contexts.map((ctx) => ctx.watch()));
  console.log("watching extension and webview bundles");
} else {
  await Promise.all([build(extension), build(webview)]);
}
