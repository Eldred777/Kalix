package com.kalix.ide.components;

import javax.swing.JComponent;
import javax.swing.JLayer;
import javax.swing.JScrollPane;
import javax.swing.JTree;
import javax.swing.JViewport;
import javax.swing.SwingUtilities;
import javax.swing.UIManager;
import javax.swing.plaf.LayerUI;
import java.awt.Color;
import java.awt.Graphics;
import java.awt.Point;
import java.awt.Rectangle;
import java.awt.event.MouseAdapter;
import java.awt.event.MouseEvent;

/**
 * Full-width hover highlight for the rows of a tree in a scroll pane. A helper cannot
 * override the tree's {@code paintComponent}, so the wash is painted by a {@link JLayer}:
 * show the layer {@link #install} returns in place of the scroll pane.
 *
 * <p>The layer goes around the scroll pane, not the tree. The look and feel reads the
 * viewport's view directly to decide whether the scroll pane holds the focus, so a layer
 * between the two would cost the scroll pane its focus border.
 */
public final class TreeHoverHighlight extends LayerUI<JScrollPane> {

    // Subtle enough that the row text stays legible.
    private static final int HOVER_ALPHA = 40;

    private final JTree tree;
    private final JViewport viewport;
    // Where the pointer is, in viewport coordinates; null while it is off the tree. The
    // viewport stays put while the tree scrolls beneath it.
    private Point pointer;
    // The row last highlighted. Re-derived from `pointer` before every paint, so a row index
    // gone stale after a scroll, expand, collapse or model change is never drawn.
    private int highlightedRow = -1;

    private TreeHoverHighlight(JTree tree, JViewport viewport) {
        this.tree = tree;
        this.viewport = viewport;
    }

    /**
     * Adds the highlight to {@code tree}, the view of {@code scrollPane}; returns the layer
     * to show in the scroll pane's place.
     */
    public static JLayer<JScrollPane> install(JTree tree, JScrollPane scrollPane) {
        TreeHoverHighlight highlight = new TreeHoverHighlight(tree, scrollPane.getViewport());

        MouseAdapter pointerTracker = new MouseAdapter() {
            @Override
            public void mouseMoved(MouseEvent e) {
                highlight.pointerMoved(e.getPoint());
            }

            @Override
            public void mouseExited(MouseEvent e) {
                highlight.pointerMoved(null);
            }
        };
        tree.addMouseMotionListener(pointerTracker);
        tree.addMouseListener(pointerTracker);
        // Scrolling moves the tree under a still pointer, so no mouse event arrives.
        highlight.viewport.addChangeListener(e -> highlight.refresh());
        return new JLayer<>(scrollPane, highlight);
    }

    @Override
    public void paint(Graphics g, JComponent c) {
        super.paint(g, c);
        // This paint may cover only a strip, so a row change seen here still needs its rows
        // repainted, not just recorded.
        refresh();
        int row = highlightedRow;
        if (row < 0 || tree.isRowSelected(row)) {
            return;
        }
        Rectangle bounds = tree.getRowBounds(row);
        if (bounds == null) {
            return;
        }
        // The wash spans the viewport and is clipped to it, clear of scroll bars and border.
        Rectangle visible = SwingUtilities.convertRectangle(
            viewport, new Rectangle(viewport.getSize()), c);
        int y = SwingUtilities.convertPoint(tree, 0, bounds.y, c).y;
        Graphics wash = g.create();
        wash.clipRect(visible.x, visible.y, visible.width, visible.height);
        wash.setColor(hoverColor());
        wash.fillRect(visible.x, y, visible.width, bounds.height);
        wash.dispose();
    }

    private void pointerMoved(Point inTree) {
        pointer = inTree == null ? null : SwingUtilities.convertPoint(tree, inTree, viewport);
        refresh();
    }

    /** Repaints only the rows the highlight left and entered. */
    private void refresh() {
        int row = rowUnderPointer();
        if (row != highlightedRow) {
            repaintRow(highlightedRow);
            repaintRow(row);
            highlightedRow = row;
        }
    }

    private int rowUnderPointer() {
        if (pointer == null) {
            return -1;
        }
        Point inTree = SwingUtilities.convertPoint(viewport, pointer, tree);
        return TreeRows.fullWidthRowAt(tree, inTree.y);
    }

    private void repaintRow(int row) {
        if (row < 0 || row >= tree.getRowCount()) {
            return;
        }
        Rectangle bounds = tree.getRowBounds(row);
        if (bounds != null) {
            tree.repaint(0, bounds.y, tree.getWidth(), bounds.height);
        }
    }

    private static Color hoverColor() {
        Color base = UIManager.getColor("Tree.selectionBackground");
        if (base == null) {
            base = Color.GRAY;
        }
        return new Color(base.getRed(), base.getGreen(), base.getBlue(), HOVER_ALPHA);
    }
}
