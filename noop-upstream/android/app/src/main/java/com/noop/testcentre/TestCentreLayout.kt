package com.noop.testcentre

/**
 * Pure projection of the registry for the Test Centre screen's section 1. Twin of the Swift
 * TestCentreLayout: shipped modes only, high before med before low (stable inside a band), and
 * requires5MG modes hidden off a non-5/MG strap (the #22 gating question). The status helper formats
 * each row's status string identically to Swift. Kept aligned by TestCentreLayoutTest.
 */
object TestCentreLayout {

    private fun rank(p: TestPriority): Int = when (p) {
        TestPriority.HIGH -> 0
        TestPriority.MED -> 1
        TestPriority.LOW -> 2
    }

    /** Order an arbitrary mode list (registry or fixture) for the screen, stable within a priority. */
    fun order(modes: List<TestMode>, is5MG: Boolean): List<TestMode> =
        modes.filter { is5MG || !it.requires5MG }
            .withIndex()
            .sortedWith(compareBy({ rank(it.value.priority) }, { it.index }))
            .map { it.value }

    /** The shipped registry projected for the current strap. Section 1 binds this. */
    fun visibleModes(is5MG: Boolean): List<TestMode> = order(TestModeRegistry.all, is5MG)

    /**
     * The row status string, twin of Swift statusText. "Off" when inactive; "On" for an active toggle
     * mode; "Capturing K of N <unit>" for an active guided mode, K = ceil(elapsed days), clamped to the
     * target so a long capture never reads past its window. No em-dash.
     */
    fun statusText(mode: TestMode, active: Boolean, elapsedSeconds: Double?): String {
        if (!active) return "Off"
        return when (val cap = mode.capture) {
            is CaptureKind.Toggle -> "On"
            is CaptureKind.Guided -> {
                val elapsed = (elapsedSeconds ?: 0.0).coerceAtLeast(0.0)
                val dayIndex = kotlin.math.ceil(elapsed / 86_400.0).toInt()
                val k = dayIndex.coerceIn(1, cap.defaultCount)
                // The mode's own word, lowercased to match the Swift CaptureUnit.rawValue
                // ("nights" / "days") so the two platforms read byte-identical.
                val unitWord = when (cap.unit) {
                    CaptureUnit.NIGHTS -> "nights"
                    CaptureUnit.DAYS -> "days"
                }
                "Capturing $k of ${cap.defaultCount} $unitWord"
            }
        }
    }
}
