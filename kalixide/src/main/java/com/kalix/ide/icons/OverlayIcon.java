package com.kalix.ide.icons;

import org.kordamp.ikonli.Ikon;
import org.kordamp.ikonli.swing.FontIcon;

import javax.swing.Icon;
import java.awt.Color;
import java.awt.Component;
import java.awt.Graphics;

/**
 * An {@link Icon} that paints one glyph directly over another at the same origin.
 *
 * <p>Ikonli's Swing pack has no composite/stacked icon support (unlike its JavaFX pack's
 * {@code StackedFontIcon}) — {@code ikonli-swing} exposes only the single-glyph
 * {@link FontIcon}. This fills that gap for icons FontAwesome doesn't ship as one combined
 * glyph, e.g. a slash over {@code MASK} to read as "masking not available" where FontAwesome
 * 6 Free has no {@code mask-slash} icon of its own.
 *
 * <p>Both layers paint at the same size and origin, so pick glyphs whose silhouettes read
 * well stacked — a solid base with a sparse, diagonal overlay (a slash) tends to work; two
 * dense glyphs will just look cluttered at menu/toolbar icon sizes.
 */
public final class OverlayIcon implements Icon {

    private final Icon base;
    private final Icon overlay;

    public OverlayIcon(Icon base, Icon overlay) {
        this.base = base;
        this.overlay = overlay;
    }

    /**
     * Builds both layers as same-sized {@link FontIcon}s, each in its own colour.
     *
     * @param baseGlyph the icon underneath (e.g. {@code FontAwesomeSolid.MASK})
     * @param baseColor colour for the base glyph
     * @param overlayGlyph the icon painted on top (e.g. {@code FontAwesomeSolid.SLASH})
     * @param overlayColor colour for the overlay glyph
     * @param size icon size in pixels, shared by both layers
     */
    public static Icon of(Ikon baseGlyph, Color baseColor, Ikon overlayGlyph, Color overlayColor, int size) {
        FontIcon baseIcon = FontIcon.of(baseGlyph, size);
        baseIcon.setIconColor(baseColor);
        FontIcon overlayIcon = FontIcon.of(overlayGlyph, size);
        overlayIcon.setIconColor(overlayColor);
        return new OverlayIcon(baseIcon, overlayIcon);
    }

    /** Convenience overload where both layers share one colour. */
    public static Icon of(Ikon baseGlyph, Ikon overlayGlyph, Color color, int size) {
        return of(baseGlyph, color, overlayGlyph, color, size);
    }

    /** Convenience overload using {@link FontIcon}'s own default colour for both layers. */
    public static Icon of(Ikon baseGlyph, Ikon overlayGlyph, int size) {
        return new OverlayIcon(FontIcon.of(baseGlyph, size), FontIcon.of(overlayGlyph, size));
    }

    @Override
    public void paintIcon(Component c, Graphics g, int x, int y) {
        base.paintIcon(c, g, x, y);
        overlay.paintIcon(c, g, x, y);
    }

    @Override
    public int getIconWidth() {
        return base.getIconWidth();
    }

    @Override
    public int getIconHeight() {
        return base.getIconHeight();
    }
}
