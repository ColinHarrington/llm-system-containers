import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/svelte";
import { describe, it, expect } from "vitest";
import Skeleton from "./Skeleton.svelte";

describe("Skeleton", () => {
  it("renders a shimmer block with the given size", () => {
    render(Skeleton, { props: { w: "50%", h: 20 } });
    const el = screen.getByTestId("skeleton");
    expect(el).toBeInTheDocument();
    expect(el).toHaveStyle({ width: "50%", height: "20px" });
  });
});
