import "./app/app.css";

import { QueryClientProvider } from "@tanstack/react-query";
import { LinenProvider } from "linen";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { RouterProvider } from "react-router/dom";

import { queryClient } from "./app/client";
import { router } from "./app/router";

const root = document.getElementById("root");
if (!root) {
  throw new Error("index.html has no #root element");
}

createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <LinenProvider>
        <RouterProvider router={router} />
      </LinenProvider>
    </QueryClientProvider>
  </StrictMode>,
);
