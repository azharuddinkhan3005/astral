export interface Logger {
    log(message: string): void;
}

export class ConsoleLogger implements Logger {
    private prefix: string;

    constructor(prefix: string = '[LOG]') {
        this.prefix = prefix;
    }

    log(message: string): void {
        console.log(`${this.prefix} ${message}`);
    }
}

export function createLogger(prefix?: string): Logger {
    return new ConsoleLogger(prefix);
}
