let spriteLoaded: Promise<void> | null = null;

export function ensureSpriteSheet(): Promise<void> {
    if (spriteLoaded) return spriteLoaded;

    spriteLoaded = fetch('/build/pieces-sprite.svg')
        .then(response => response.text())
        .then(svgText => {
            if (!document.getElementById('keres-sprite-sheet')) {
                const div = document.createElement('div');
                div.id = 'keres-sprite-sheet';
                div.innerHTML = svgText;
                document.body.insertBefore(div, document.body.firstChild);
            }
        });

    return spriteLoaded;
}
