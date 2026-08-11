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

sealed class CapabilityRecommendationState {
  const CapabilityRecommendationState();
}

final class CapabilityRecommendationIdle extends CapabilityRecommendationState {
  const CapabilityRecommendationIdle();
}

final class CapabilityRecommendationLoading
    extends CapabilityRecommendationState {
  const CapabilityRecommendationLoading();
}

final class CapabilityRecommendationReady
    extends CapabilityRecommendationState {
  const CapabilityRecommendationReady({required this.report});

  final CapabilityReport report;
}

final class CapabilityRecommendationNoPlan
    extends CapabilityRecommendationState {
  const CapabilityRecommendationNoPlan({required this.report});

  final CapabilityReport report;
}

final class CapabilityRecommendationFailed
    extends CapabilityRecommendationState {
  const CapabilityRecommendationFailed({required this.diagnosticCode});

  final String diagnosticCode;
}

sealed class RuntimeStatusState {
  const RuntimeStatusState();
}

final class RuntimeStatusIdle extends RuntimeStatusState {
  const RuntimeStatusIdle();
}

final class RuntimeStatusLoading extends RuntimeStatusState {
  const RuntimeStatusLoading({this.previousReport});

  final RuntimeHealthReport? previousReport;
}

final class RuntimeStatusLoaded extends RuntimeStatusState {
  const RuntimeStatusLoaded({required this.report});

  final RuntimeHealthReport report;
}

final class RuntimeStatusOperating extends RuntimeStatusState {
  const RuntimeStatusOperating({required this.report, required this.kind});

  final RuntimeHealthReport report;
  final RuntimeOperationKind kind;
}

final class RuntimeStatusFailed extends RuntimeStatusState {
  const RuntimeStatusFailed({
    required this.diagnosticCode,
    this.report,
    this.operationKind,
    this.recoveryAction = RecoveryAction.retry,
  });

  final String diagnosticCode;
  final RuntimeHealthReport? report;
  final RuntimeOperationKind? operationKind;
  final RecoveryAction recoveryAction;
}

final class RuntimeStatusCancelled extends RuntimeStatusState {
  const RuntimeStatusCancelled({this.report});

  final RuntimeHealthReport? report;
}

sealed class RuntimeInventoryState {
  const RuntimeInventoryState();
}

final class RuntimeInventoryIdle extends RuntimeInventoryState {
  const RuntimeInventoryIdle();
}

final class RuntimeInventoryLoading extends RuntimeInventoryState {
  const RuntimeInventoryLoading();
}

final class RuntimeInventoryLoaded extends RuntimeInventoryState {
  const RuntimeInventoryLoaded({required this.inventory});

  final RuntimeModelInventory inventory;
}

final class RuntimeInventoryFailed extends RuntimeInventoryState {
  const RuntimeInventoryFailed({
    required this.diagnosticCode,
    this.recoveryAction = RecoveryAction.retry,
  });

  final String diagnosticCode;
  final RecoveryAction recoveryAction;
}
