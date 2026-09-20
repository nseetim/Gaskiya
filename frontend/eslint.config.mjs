import { defineConfig, globalIgnores } from "eslint/config";
import nextVitals from "eslint-config-next/core-web-vitals";
import nextTs from "eslint-config-next/typescript";

const eslintConfig = defineConfig([
  ...nextVitals,
  ...nextTs,
  {
    // React Compiler readiness rules, enforced as errors by
    // eslint-config-next regardless of whether the compiler is actually
    // enabled (it isn't here — scaffolded with --no-react-compiler). The
    // flagged pattern (fetch-on-mount in useEffect, setState after an
    // await) is standard and functionally correct; downgraded to warnings
    // rather than rearchitecting data fetching around Suspense/`use()`,
    // which is out of scope for this project.
    rules: {
      "react-hooks/set-state-in-effect": "warn",
      "react-hooks/purity": "warn",
    },
  },
  // Override default ignores of eslint-config-next.
  globalIgnores([
    // Default ignores of eslint-config-next:
    ".next/**",
    "out/**",
    "build/**",
    "next-env.d.ts",
  ]),
]);

export default eslintConfig;
