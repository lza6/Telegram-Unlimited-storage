import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}", "src/**/**/*.test.{ts,tsx}"],
    setupFiles: ["./src/test-setup.ts"],
    globals: true,
    coverage: {
      provider: "v8",
      include: [
        "src/lib/*Pure.ts",
        "src/lib/webMetaPure.ts",
        "src/lib/downloadPure.ts",
        "src/lib/queuePure.ts",
        "src/lib/transferUiPure.ts",
        "src/hooks/useFileUpload.ts",
        "src/hooks/useFileDownload.ts",
        "src/hooks/useFileOperations.ts",
        "src/hooks/useTelegramConnection.ts",
        "src/utils.ts",
        "src/types/connection.ts",
      ],
      thresholds: {
        lines: 80,
        functions: 80,
        branches: 80,
        statements: 80,
      },
    },
  },
});
