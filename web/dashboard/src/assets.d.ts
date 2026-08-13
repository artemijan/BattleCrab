// TS 7 (TS2882) requires even side-effect imports of non-TS files to resolve
// to *something*. The bundler handles these at build time; this tells the
// type-checker they exist.
declare module "*.css";
declare module "*.svg" {
  const url: string;
  export default url;
}
declare module "*.webp" {
  const url: string;
  export default url;
}
declare module "*.png" {
  const url: string;
  export default url;
}
declare module "*.jpg" {
  const url: string;
  export default url;
}
// JSON is imported `with { type: "file" }` (a URL to fetch lazily), never
// inlined — see ItemIcon.tsx, whose atlas map is far too big to bundle.
declare module "*.json" {
  const url: string;
  export default url;
}
