package com.kalix.ide.flowviz.rendering;

import com.kalix.ide.flowviz.transform.PlotType;
import com.kalix.ide.icons.OverlayIcon;
import com.kalix.ide.utils.ThemeUtils;

import org.kordamp.ikonli.fontawesome6.FontAwesomeSolid;
import org.kordamp.ikonli.swing.FontIcon;

import javax.swing.Icon;
import javax.swing.JLabel;
import javax.swing.JList;
import javax.swing.ListCellRenderer;

import java.awt.Color;
import java.awt.Component;

import static com.kalix.ide.windows.ToolbarConstants.BUTTON_ICON_SIZE;
import static com.kalix.ide.windows.ToolbarConstants.HORIZONTAL_SPACING;

/**
 * Renders each {@link PlotType} combo-box item as its display name plus a mask glyph showing
 * whether that plot type starts with overlapping-data masking on
 * ({@link PlotType#isDataMaskDefault()}) — a plain mask when it does, a slashed mask when it
 * doesn't. FontAwesome 6 Free ships no "mask-slash" glyph, so the slashed version is
 * composited via {@link OverlayIcon}.
 */
public class PlotTypeListCellRenderer implements ListCellRenderer<PlotType> {

    @Override
    public Component getListCellRendererComponent(
        JList<? extends PlotType> list,
        PlotType value,
        int index,
        boolean isSelected,
        boolean cellHasFocus
    ) {
        Color background = isSelected ? list.getSelectionBackground() : list.getBackground();
        Color foreground = isSelected ? list.getSelectionForeground() : list.getForeground();

        JLabel label;
        if (value == null) {
            label = new JLabel();
        } else {
            // Icon colour follows the row's own background (selected vs. not) rather than a
            // fixed colour, so the glyph stays legible across both light and dark themes and
            // across the highlighted/unhighlighted state within the same popup.
            Color iconColor = ThemeUtils.iconColor(background);
            Icon icon = value.isDataMaskDefault()
                ? FontIcon.of(FontAwesomeSolid.MASK, BUTTON_ICON_SIZE, iconColor)
                : OverlayIcon.of(FontAwesomeSolid.MASK, iconColor, FontAwesomeSolid.SLASH, iconColor, BUTTON_ICON_SIZE);
            label = new JLabel(value.getDisplayName(), icon, JLabel.LEFT);
            label.setIconTextGap(HORIZONTAL_SPACING);
        }

        // A JLabel paints no background unless told to — without this, isSelected/foreground
        // below would have no visible effect and the popup would show no highlighted row.
        label.setOpaque(true);
        label.setBackground(background);
        label.setForeground(foreground);
        return label;
    }
}
