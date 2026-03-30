/**
 * Scrollable viewport -- tracks visible slice of a list.
 */

export class ScrollState {
  private _offset = 0;
  private _selected = 0;
  private _total: number;
  private _viewportHeight: number;

  constructor(total: number, viewportHeight: number) {
    this._total = Math.max(0, total);
    this._viewportHeight = Math.max(1, viewportHeight);
  }

  get offset(): number { return this._offset; }
  get selected(): number { return this._selected; }
  get total(): number { return this._total; }
  get viewportHeight(): number { return this._viewportHeight; }

  /** Visible range: [start, end) indices into the list. */
  get visibleRange(): { start: number; end: number } {
    const start = this._offset;
    const end = Math.min(this._offset + this._viewportHeight, this._total);
    return { start, end };
  }

  /** Move selection down by 1. Returns true if changed. */
  moveDown(): boolean {
    if (this._selected >= this._total - 1) return false;
    this._selected++;
    this.ensureVisible();
    return true;
  }

  /** Move selection up by 1. Returns true if changed. */
  moveUp(): boolean {
    if (this._selected <= 0) return false;
    this._selected--;
    this.ensureVisible();
    return true;
  }

  /** Set selection directly. */
  setSelected(index: number): void {
    this._selected = Math.max(0, Math.min(index, this._total - 1));
    this.ensureVisible();
  }

  /** Update total count (e.g., list changed). Clamps selection. */
  setTotal(total: number): void {
    this._total = Math.max(0, total);
    if (this._selected >= this._total) {
      this._selected = Math.max(0, this._total - 1);
    }
    this.ensureVisible();
  }

  /** Update viewport height (e.g., resize). */
  setViewportHeight(height: number): void {
    this._viewportHeight = Math.max(1, height);
    this.ensureVisible();
  }

  private ensureVisible(): void {
    if (this._selected < this._offset) {
      this._offset = this._selected;
    }
    if (this._selected >= this._offset + this._viewportHeight) {
      this._offset = this._selected - this._viewportHeight + 1;
    }
  }
}
