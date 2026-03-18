<?php

namespace App;

class Calculator {
    public function add(float $a, float $b): float {
        return $a + $b;
    }

    public function multiply(float $a, float $b): float {
        return $a * $b;
    }

    public function divide(float $a, float $b): ?float {
        if ($b == 0) {
            return null;
        }
        return $a / $b;
    }
}

function factorial(int $n): int {
    if ($n <= 1) return 1;
    return $n * factorial($n - 1);
}
