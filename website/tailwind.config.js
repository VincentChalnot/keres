/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./layouts/**/*.html", "./i18n/**/*.toml"],
  theme: {
    extend: {
      colors: {
        keres: {
          primary: "#e19e5b",
          secondary: "#e1c499",
          light: "#f8f0e6",
          dark: "#55442d",
          bg: "#010101",
          surface: "#1a1a1a",
        },
      },
      fontFamily: {
        carolingia: ['Carolingia', 'serif'],
        roman: ['RomanSerif', 'serif'],
      },
      fontSize: {
        "h1": ["6rem", { lineHeight: "1.1" }],
        "h1-sm": ["8rem", { lineHeight: "1.1" }],
        "h2": ["3rem", { lineHeight: "1.1" }],
        "h3": ["2rem", { lineHeight: "1.1" }],
        "h4": ["1.5rem", { lineHeight: "1.1" }],
        "h5": ["1.2rem", { lineHeight: "1.1" }],
      },
      borderRadius: {
        rounded: "9999px",
      },
      maxWidth: {
        hero: "60ch",
        section: "900px",
      },
    },
  },
  plugins: [require("@tailwindcss/typography")],
};
