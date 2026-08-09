import 'package:gixgiz_desktop/core/core_client.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';

sealed class FoundationState {
  const FoundationState();
}

final class FoundationLoading extends FoundationState {
  const FoundationLoading();
}

final class FoundationReady extends FoundationState {
  const FoundationReady({required this.snapshot});

  final CoreConnectionSnapshot snapshot;
}

final class FoundationDegraded extends FoundationState {
  const FoundationDegraded({required this.snapshot});

  final CoreConnectionSnapshot snapshot;
}

final class FoundationUnavailable extends FoundationState {
  const FoundationUnavailable({
    required this.issue,
    required this.diagnosticCode,
  });

  final CoreConnectionIssue issue;
  final String diagnosticCode;
}

final class FoundationFailed extends FoundationState {
  const FoundationFailed({required this.issue, required this.diagnosticCode});

  final CoreConnectionIssue issue;
  final String diagnosticCode;
}

final class FoundationCancelled extends FoundationState {
  const FoundationCancelled();
}

sealed class HardwareScanState {
  const HardwareScanState();
}

final class HardwareScanIdle extends HardwareScanState {
  const HardwareScanIdle();
}

final class HardwareScanLoading extends HardwareScanState {
  const HardwareScanLoading();
}

final class HardwareScanReady extends HardwareScanState {
  const HardwareScanReady({required this.profile});

  final MachineProfile profile;
}

final class HardwareScanPartial extends HardwareScanState {
  const HardwareScanPartial({required this.profile});

  final MachineProfile profile;
}

final class HardwareScanFailed extends HardwareScanState {
  const HardwareScanFailed({required this.diagnosticCode});

  final String diagnosticCode;
}

final class HardwareScanCancelled extends HardwareScanState {
  const HardwareScanCancelled();
}
