import type { Config } from "tailwindcss";

function withOpacity(variableName: string) {
  return ({ opacityValue }: { opacityValue?: string }) => {
    if (opacityValue !== undefined) {
      return `color-mix(in oklch, var(${variableName}) calc(100% * ${opacityValue}), transparent)`;
    }
    return `var(${variableName})`;
  };
}

const config: Config = {
  darkMode: ["class"],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        border: withOpacity("--border"),
        input: withOpacity("--input"),
        ring: withOpacity("--ring"),
        background: withOpacity("--background"),
        foreground: withOpacity("--foreground"),
        primary: {
          DEFAULT: withOpacity("--primary"),
          foreground: withOpacity("--primary-foreground")
        },
        muted: {
          DEFAULT: withOpacity("--muted"),
          foreground: withOpacity("--muted-foreground")
        },
        accent: {
          DEFAULT: withOpacity("--accent"),
          foreground: withOpacity("--accent-foreground")
        },
        destructive: {
          DEFAULT: withOpacity("--destructive"),
          foreground: withOpacity("--destructive-foreground")
        },
        warning: {
          DEFAULT: withOpacity("--warning")
        },
        card: {
          DEFAULT: withOpacity("--card"),
          foreground: withOpacity("--card-foreground")
        },
        violet: {
          50: withOpacity("--violet-50"),
          200: withOpacity("--violet-200"),
          400: withOpacity("--violet-400"),
          500: withOpacity("--violet-500"),
          800: withOpacity("--violet-800"),
          900: withOpacity("--violet-900"),
          950: withOpacity("--violet-950")
        }
      },
      borderRadius: {
        lg: "8px",
        md: "6px",
        sm: "4px"
      }
    }
  },
  plugins: []
};

export default config;
