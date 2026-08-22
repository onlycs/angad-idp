import * as colors from "@carbon/colors";
import * as layout from "@carbon/layout";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

const OUT = fileURLToPath(new URL("../app/style/palette.css", import.meta.url));

const kebab = (s: string) => s.replace(/([a-z])([A-Z])/g, "$1-$2").toLowerCase();
const out = ["@theme {", "  --color-*: initial;", "  --spacing: initial;"];

for (const [name, value] of Object.entries(colors)) {
    if (typeof value !== "string") continue;
    const m = name.match(/^([a-zA-Z]+?)(\d+)$/);
    const mHover = name.match(/^([a-zA-Z]+?)(\d+)(Hover)$/);

    if (mHover) {
        out.push(`  --color-${kebab(mHover[1])}-${mHover[2]}-hover: ${value};`);
    } else if (m) {
        out.push(`  --color-${kebab(m[1])}-${m[2]}: ${value};`);
    } else {
        out.push(`  --color-${kebab(name)}: ${value};`);
    }
}

Object.entries(layout)
    .filter(([k, v]) => /^spacing\d{2}$/.test(k) && typeof v === "string")
    .map(([k, v]) => `  --spacing-${k.slice(-2)}: ${v};`)
    .forEach((item) => out.push(item));

out.push("}");

mkdirSync(dirname(OUT), { recursive: true });
writeFileSync(OUT, out.join("\n") + "\n");
