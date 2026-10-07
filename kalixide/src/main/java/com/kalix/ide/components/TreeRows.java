package com.kalix.ide.components;

import javax.swing.JTree;
import javax.swing.tree.TreePath;
import java.awt.Rectangle;

/**
 * Row hit-testing for trees whose rows read as full-width (hover, selection, context menus).
 * {@link JTree#getPathForLocation} and {@link JTree#getRowForLocation} only hit the renderer's
 * label-width bounds, so a click beside a short label misses its row.
 */
public final class TreeRows {

    private TreeRows() {
    }

    /** The row at {@code y}, whatever the x; -1 above the first row or below the last. */
    public static int fullWidthRowAt(JTree tree, int y) {
        int row = tree.getClosestRowForLocation(0, y);
        if (row < 0) {
            return -1;
        }
        Rectangle bounds = tree.getRowBounds(row);
        return (bounds != null && y >= bounds.y && y < bounds.y + bounds.height) ? row : -1;
    }

    /** The path of the row at {@code y}, whatever the x; null where there is no row. */
    public static TreePath fullWidthPathAt(JTree tree, int y) {
        int row = fullWidthRowAt(tree, y);
        return row < 0 ? null : tree.getPathForRow(row);
    }

    /**
     * Updates the selection for a right-click at {@code y} and returns the row hit, or -1.
     * A row outside the selection becomes the selection; a row inside it leaves a
     * multi-selection intact, so the menu acts on all of it; empty space clears it.
     */
    public static int selectForPopup(JTree tree, int y) {
        int row = fullWidthRowAt(tree, y);
        if (row < 0) {
            tree.clearSelection();
        } else if (!tree.isRowSelected(row)) {
            tree.setSelectionRow(row);
        }
        return row;
    }
}
