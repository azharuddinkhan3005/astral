import { Logger } from './logger';

export interface GreetOptions {
    loud?: boolean;
}

export function greet(name: string, options?: GreetOptions): string {
    const greeting = `Hello, ${name}!`;
    if (options?.loud) {
        return greeting.toUpperCase();
    }
    return greeting;
}

export class Greeter {
    private logger: Logger;

    constructor(logger: Logger) {
        this.logger = logger;
    }

    greet(name: string): string {
        const msg = greet(name);
        this.logger.log(msg);
        return msg;
    }

    greetAll(names: string[]): string[] {
        return names.map((name) => this.greet(name));
    }
}
