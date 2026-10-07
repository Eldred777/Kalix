package com.kalix.ide.components;

import javax.swing.JComponent;
import javax.swing.JLayer;
import javax.swing.JTree;
import javax.swing.SwingUtilities;
import javax.swing.UIManager;
import javax.swing.plaf.LayerUI;
import java.awt.Color;
import java.awt.Component;
import java.awt.Graphics;
import java.awt.Point;
import java.awt.Rectangle;
import java.awt.event.ComponentAdapter;
import java.awt.event.ComponentEvent;
import java.awt.event.MouseAdapter;
import java.awt.event.MouseEvent;

/**
 * Full-width hover highlight for a tree's rows. A helper cannot override the tree's
 * {@code paintComponent}, so the wash is painted by a {@link JLayer} over the tree: put the
 * layer {@link #install} returns in the scroll pane, in place of the tree.
 */
public final class TreeHoverHighlight extends LayerUI<JTree> {

    // Subtle enough that the row text stays legible.
    private static final int HOVER_ALPHA = 40;

    private final JTree tree;
    private JLayer<JTree> layer;
    // Where the pointer is, in the layer's parent's coordinates (the viewport, when scrolled);
    // null while it is off the tree. That frame stays put while the tree scrolls beneath it.
    private Point pointer;
    // The row last highlighted. Re-derived from `pointer` before every paint, so a row index
    // gone stale after a scroll, expand, collapse or model change is never drawn.
    private int highlightedRow = -1;

    private TreeHoverHighlight(JTree tree) {
        this.tree = tree;
    }

    /** Adds the highlight to {@code tree}; returns the layer to show in its place. */
    public static JLayer<JTree> install(JTree tree) {
        TreeHoverHighlight highlight = new TreeHoverHighlight(tree);
        JLayer<JTree> layer = new JLayer<>(tree, highlight);
        highlight.layer = layer;

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
        // Scrolling moves the layer under a still pointer, so no mouse event arrives.
        layer.addComponentListener(new ComponentAdapter() {
            @Override
            public void componentMoved(ComponentEvent e) {
                highlight.refresh();
            }

            @Override
            public void componentResized(ComponentEvent e) {
                highlight.refresh();
            }
        });
        return layer;
    }

    @Override
    public void paint(Graphics g, JComponent c) {
        super.paint(g, c);
        // This paint may cover only a strip (a blit scroll paints just what it exposed), so
        // a row change seen here still needs its rows repainted, not just recorded.
        refresh();
        int row = highlightedRow;
        if (row < 0 || tree.isRowSelected(row)) {
            return;
        }
        Rectangle bounds = tree.getRowBounds(row);
        if (bounds != null) {
            g.setColor(hoverColor());
            g.fillRect(0, bounds.y, c.getWidth(), bounds.height);
        }
    }

    private void pointerMoved(Point inTree) {
        pointer = inTree == null ? null : SwingUtilities.convertPoint(tree, inTree, pointerFrame());
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
        Point inTree = SwingUtilities.convertPoint(pointerFrame(), pointer, tree);
        return TreeRows.fullWidthRowAt(tree, inTree.y);
    }

    private Component pointerFrame() {
        Component parent = layer.getParent();
        return parent != null ? parent : layer;
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
