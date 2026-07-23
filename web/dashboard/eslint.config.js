// ESLint here exists for exactly one thing: Tailwind class linting, which
// Biome cannot do. Biome stays the formatter and general linter; keep this
// config to `better-tailwindcss` rules only, so the two tools never disagree.
import betterTailwindcss from "eslint-plugin-better-tailwindcss";
import tseslint from "typescript-eslint";

export default [
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    plugins: { "better-tailwindcss": betterTailwindcss },
    settings: {
      "better-tailwindcss": {
        // Tailwind v4 has no config file; the theme lives in the CSS entry.
        entryPoint: "src/styles/globals.css",
        // The `cx()` helper joins class fragments across the codebase. The
        // explicit matcher makes every string literal anywhere inside the call
        // count — including ternary branches, which the bare form skips.
        callees: [["cx", [{ match: "strings" }]]],
        // Class strings also live in plain consts (`base`, `variants` in
        // ui.tsx) and in className-returning arrow functions (NavLink).
        variables: [
          ["base", [{ match: "strings" }]],
          ["variants", [{ match: "objectValues" }]],
          ["styles", [{ match: "strings" }]],
          ["className", [{ match: "strings" }]],
        ],
      },
    },
    rules: {
      // The two classes of real bug: the same property written twice with
      // different values (last-in-CSS wins, not last-in-string — a classic
      // silent override), and literal duplicates left behind by refactors.
      "better-tailwindcss/no-conflicting-classes": "error",
      "better-tailwindcss/no-duplicate-classes": "error",
      // Catches typos (`text-mutedd`) and classes that silently compile to
      // nothing. Custom classes defined in globals.css are declared here so
      // the rule knows they are real.
      ["better-tailwindcss/no-unknown-classes"]: [
        "error",
        { ignore: ["glass", "glass-sheen", "bg-field", "animate-rise", "stagger", "dark"] },
      ],
      // `[transition-timing-function:var(--ease-out-soft)]` → `ease-out-soft`:
      // arbitrary values that already exist as theme utilities must use them.
      "better-tailwindcss/enforce-canonical-classes": "error",
      // `mt-1 mb-1` → `my-1`.
      "better-tailwindcss/enforce-shorthand-classes": "error",
    },
  },
];
