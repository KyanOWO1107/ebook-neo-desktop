// @vitest-environment jsdom

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { VirtualList } from "./VirtualList";

describe("VirtualList", () => {
  it("renders only visible rows for a large fixed-height list", () => {
    render(
      <VirtualList
        ariaLabel="large virtual list"
        itemCount={1000}
        rowHeight={32}
        height={160}
        overscan={1}
        renderRow={(index) => <div data-testid="virtual-row">Row {index}</div>}
      />,
    );

    expect(screen.getByText("Row 0")).toBeTruthy();
    expect(screen.queryByText("Row 999")).toBeNull();
    expect(screen.getAllByTestId("virtual-row").length).toBeLessThan(12);
    expect(screen.getByLabelText("large virtual list").getAttribute("data-total-items")).toBe("1000");
  });
});
