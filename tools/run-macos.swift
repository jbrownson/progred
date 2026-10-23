import AppKit

guard CommandLine.arguments.count == 2 else {
    fputs("usage: run-macos.swift <application bundle>\n", stderr)
    exit(2)
}

var application: NSRunningApplication?
var termination: NSKeyValueObservation?
var stopping = false

func stopApplication() {
    stopping = true
    if let application, !application.isTerminated, !application.forceTerminate() {
        fputs("could not stop the development application\n", stderr)
        exit(1)
    }
}

let signals = [SIGINT, SIGQUIT, SIGTERM, SIGHUP].map { number in
    signal(number, SIG_IGN)
    let source = DispatchSource.makeSignalSource(signal: number, queue: .main)
    source.setEventHandler(handler: stopApplication)
    source.resume()
    return source
}

let configuration = NSWorkspace.OpenConfiguration()
configuration.createsNewApplicationInstance = true
NSWorkspace.shared.openApplication(
    at: URL(fileURLWithPath: CommandLine.arguments[1]),
    configuration: configuration
) { running, error in
    DispatchQueue.main.async {
        guard let running else {
            fputs("could not launch Progred: \(error?.localizedDescription ?? "unknown error")\n", stderr)
            exit(1)
        }
        application = running
        termination = running.observe(\.isTerminated, options: [.initial, .new]) { running, _ in
            if running.isTerminated {
                exit(0)
            }
        }
        if stopping {
            stopApplication()
        }
    }
}
RunLoop.main.run()
