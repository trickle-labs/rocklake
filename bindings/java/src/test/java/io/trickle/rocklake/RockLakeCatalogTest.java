package io.trickle.rocklake;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Path;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class RockLakeCatalogTest {

    @TempDir
    Path tempDir;

    @Test
    void openCloseSmoke() throws Exception {
        try (RockLakeCatalog catalog = new RockLakeCatalog(tempDir.toString())) {
            assertTrue(catalog.isOpen());
            assertEquals(tempDir.toString(), catalog.getPath());
        }
    }

    @Test
    void openRejectsEmptyPath() {
        assertThrows(IllegalArgumentException.class, () -> new RockLakeCatalog(""));
    }

    @Test
    void openReadOnlyRejectsEmptyPath() {
        assertThrows(IllegalArgumentException.class, () -> RockLakeCatalog.openReadOnly(""));
    }

    @Test
    void closeIsIdempotent() throws Exception {
        RockLakeCatalog catalog = new RockLakeCatalog(tempDir.toString());
        catalog.close();
        catalog.close();
        assertFalse(catalog.isOpen());
    }
}