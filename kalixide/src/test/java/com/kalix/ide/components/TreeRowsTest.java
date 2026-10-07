package com.kalix.ide.components;

import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import javax.swing.JTree;
import javax.swing.tree.DefaultMutableTreeNode;
import javax.swing.tree.DefaultTreeModel;
import javax.swing.tree.TreeSelectionModel;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

/**
 * {@link TreeRows} against a laid-out tree of four 20px rows (root, a, b, c) in a 400px
 * wide component, so every row has empty space to the right of its label.
 */
class TreeRowsTest {

    private static final int ROW_HEIGHT = 20;
    private static final int WIDTH = 400;

    private JTree tree;

    @BeforeEach
    void setUp() {
        DefaultMutableTreeNode root = new DefaultMutableTreeNode("root");
        root.add(new DefaultMutableTreeNode("a"));
        root.add(new DefaultMutableTreeNode("b"));
        root.add(new DefaultMutableTreeNode("c"));
        tree = new JTree(new DefaultTreeModel(root));
        tree.setRowHeight(ROW_HEIGHT);
        tree.getSelectionModel().setSelectionMode(TreeSelectionModel.DISCONTIGUOUS_TREE_SELECTION);
        tree.setSize(WIDTH, 10 * ROW_HEIGHT);
    }

    private static int middleOf(int row) {
        return row * ROW_HEIGHT + ROW_HEIGHT / 2;
    }

    @Test
    void rowIsHitBesideItsLabel() {
        int y = middleOf(2);
        // The stock hit test misses here, which is the reason TreeRows exists.
        assertNull(tree.getPathForLocation(WIDTH - 5, y));

        assertEquals(2, TreeRows.fullWidthRowAt(tree, y));
        assertEquals(tree.getPathForRow(2), TreeRows.fullWidthPathAt(tree, y));
    }

    @Test
    void rowEdgesBelongToOneRowEach() {
        assertEquals(1, TreeRows.fullWidthRowAt(tree, ROW_HEIGHT));
        assertEquals(1, TreeRows.fullWidthRowAt(tree, 2 * ROW_HEIGHT - 1));
        assertEquals(2, TreeRows.fullWidthRowAt(tree, 2 * ROW_HEIGHT));
    }

    @Test
    void noRowBelowTheLastOrAboveTheFirst() {
        assertEquals(-1, TreeRows.fullWidthRowAt(tree, 4 * ROW_HEIGHT));
        assertEquals(-1, TreeRows.fullWidthRowAt(tree, -1));
        assertNull(TreeRows.fullWidthPathAt(tree, 4 * ROW_HEIGHT));
    }

    @Test
    void popupOnUnselectedRowSelectsOnlyThatRow() {
        tree.setSelectionRows(new int[] {1, 2});

        assertEquals(3, TreeRows.selectForPopup(tree, middleOf(3)));
        assertArrayEquals(new int[] {3}, tree.getSelectionRows());
    }

    @Test
    void popupInsideMultiSelectionKeepsIt() {
        tree.setSelectionRows(new int[] {1, 2});

        assertEquals(2, TreeRows.selectForPopup(tree, middleOf(2)));
        assertArrayEquals(new int[] {1, 2}, tree.getSelectionRows());
    }

    @Test
    void popupOnEmptySpaceClearsSelection() {
        tree.setSelectionRows(new int[] {1, 2});

        assertEquals(-1, TreeRows.selectForPopup(tree, 6 * ROW_HEIGHT));
        assertEquals(0, tree.getSelectionCount());
    }
}
