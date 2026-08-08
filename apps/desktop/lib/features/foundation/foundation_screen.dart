import 'package:flutter/material.dart';
import 'package:gixgiz_desktop/app/app_keys.dart';
import 'package:gixgiz_desktop/core/core_client.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/features/foundation/foundation_state.dart';
import 'package:gixgiz_desktop/l10n/app_localizations.dart';
import 'package:gixgiz_desktop/shared/page_header.dart';

class FoundationScreen extends StatelessWidget {
  const FoundationScreen({
    required this.state,
    required this.hardwareScanState,
    required this.onRetry,
    required this.onStartHardwareScan,
    required this.onCancelHardwareScan,
    this.primaryActionFocusNode,
    super.key,
  });

  final FoundationState state;
  final HardwareScanState hardwareScanState;
  final VoidCallback onRetry;
  final VoidCallback onStartHardwareScan;
  final VoidCallback onCancelHardwareScan;
  final FocusNode? primaryActionFocusNode;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final status = _statusFor(localizations, state);

    return SafeArea(
      child: FocusTraversalGroup(
        policy: OrderedTraversalPolicy(),
        child: SingleChildScrollView(
          padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 28),
          child: Align(
            alignment: Alignment.topCenter,
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 840),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  PageHeader(sectionTitle: localizations.foundationTitle),
                  const SizedBox(height: 32),
                  _StatusPanel(
                    status: status,
                    state: state,
                    onRetry: onRetry,
                    primaryActionFocusNode: primaryActionFocusNode,
                  ),
                  if (state is FoundationReady ||
                      state is FoundationDegraded) ...[
                    const SizedBox(height: 24),
                    _HardwareScanPanel(
                      state: hardwareScanState,
                      onStart: onStartHardwareScan,
                      onCancel: onCancelHardwareScan,
                    ),
                  ],
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }

  _FoundationStatus _statusFor(
    AppLocalizations localizations,
    FoundationState state,
  ) {
    return switch (state) {
      FoundationLoading() => _FoundationStatus(
        title: localizations.loadingTitle,
        message: localizations.loadingMessage,
        icon: Icons.sync,
        tone: _StatusTone.neutral,
      ),
      FoundationReady(:final snapshot) => _FoundationStatus(
        title: localizations.coreReadyTitle,
        message: snapshot.readinessSummary ?? localizations.coreReadyMessage,
        icon: Icons.check_circle_outline,
        tone: _StatusTone.ready,
      ),
      FoundationDegraded(:final snapshot) => _FoundationStatus(
        title: localizations.degradedTitle,
        message: snapshot.readinessSummary ?? localizations.degradedMessage,
        icon: Icons.warning_amber,
        tone: _StatusTone.warning,
      ),
      FoundationUnavailable(:final issue) => _unavailableStatus(
        localizations,
        issue,
      ),
      FoundationFailed(:final issue) => _failedStatus(localizations, issue),
      FoundationCancelled() => _FoundationStatus(
        title: localizations.cancelledTitle,
        message: localizations.cancelledMessage,
        icon: Icons.cancel_outlined,
        tone: _StatusTone.neutral,
      ),
    };
  }

  _FoundationStatus _unavailableStatus(
    AppLocalizations localizations,
    CoreConnectionIssue issue,
  ) {
    return switch (issue) {
      CoreConnectionIssue.missingCore => _FoundationStatus(
        title: localizations.missingCoreTitle,
        message: localizations.missingCoreMessage,
        icon: Icons.extension_off_outlined,
        tone: _StatusTone.error,
      ),
      CoreConnectionIssue.startupTimedOut => _FoundationStatus(
        title: localizations.startupTimedOutTitle,
        message: localizations.startupTimedOutMessage,
        icon: Icons.timer_off_outlined,
        tone: _StatusTone.warning,
      ),
      CoreConnectionIssue.connectionLost => _FoundationStatus(
        title: localizations.connectionLostTitle,
        message: localizations.connectionLostMessage,
        icon: Icons.link_off,
        tone: _StatusTone.warning,
      ),
      _ => _FoundationStatus(
        title: localizations.notConnectedTitle,
        message: localizations.notConnectedMessage,
        icon: Icons.link_off,
        tone: _StatusTone.warning,
      ),
    };
  }

  _FoundationStatus _failedStatus(
    AppLocalizations localizations,
    CoreConnectionIssue issue,
  ) {
    return switch (issue) {
      CoreConnectionIssue.protocolMismatch => _FoundationStatus(
        title: localizations.protocolMismatchTitle,
        message: localizations.protocolMismatchMessage,
        icon: Icons.system_update_alt,
        tone: _StatusTone.error,
      ),
      CoreConnectionIssue.authenticationFailed => _FoundationStatus(
        title: localizations.authenticationFailedTitle,
        message: localizations.authenticationFailedMessage,
        icon: Icons.lock_outline,
        tone: _StatusTone.error,
      ),
      _ => _FoundationStatus(
        title: localizations.failedTitle,
        message: localizations.failedMessage,
        icon: Icons.error_outline,
        tone: _StatusTone.error,
      ),
    };
  }
}

class _HardwareScanPanel extends StatelessWidget {
  const _HardwareScanPanel({
    required this.state,
    required this.onStart,
    required this.onCancel,
  });

  final HardwareScanState state;
  final VoidCallback onStart;
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final colorScheme = Theme.of(context).colorScheme;
    final status = switch (state) {
      HardwareScanIdle() => (
        localizations.hardwareScanIdleTitle,
        localizations.hardwareScanIdleMessage,
        Icons.memory_outlined,
        colorScheme.primary,
      ),
      HardwareScanLoading() => (
        localizations.hardwareScanLoadingTitle,
        localizations.hardwareScanLoadingMessage,
        Icons.manage_search,
        colorScheme.primary,
      ),
      HardwareScanReady() => (
        localizations.hardwareScanReadyTitle,
        localizations.hardwareScanReadyMessage,
        Icons.check_circle_outline,
        colorScheme.primary,
      ),
      HardwareScanPartial() => (
        localizations.hardwareScanPartialTitle,
        localizations.hardwareScanPartialMessage,
        Icons.warning_amber,
        colorScheme.tertiary,
      ),
      HardwareScanFailed() => (
        localizations.hardwareScanFailedTitle,
        localizations.hardwareScanFailedMessage,
        Icons.error_outline,
        colorScheme.error,
      ),
      HardwareScanCancelled() => (
        localizations.hardwareScanCancelledTitle,
        localizations.hardwareScanCancelledMessage,
        Icons.cancel_outlined,
        colorScheme.secondary,
      ),
    };
    final profile = switch (state) {
      HardwareScanReady(:final profile) ||
      HardwareScanPartial(:final profile) => profile,
      _ => null,
    };

    return Card(
      key: AppKeys.hardwareScan,
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Semantics(
              key: AppKeys.hardwareScanStatus,
              container: true,
              liveRegion: true,
              label: localizations.hardwareScanStatusSemanticLabel(
                status.$1,
                status.$2,
              ),
              child: ExcludeSemantics(
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Icon(status.$3, color: status.$4, size: 28),
                    const SizedBox(width: 16),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            status.$1,
                            style: Theme.of(context).textTheme.titleMedium,
                          ),
                          const SizedBox(height: 6),
                          Text(status.$2),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
            if (state is HardwareScanLoading) ...[
              const SizedBox(height: 20),
              Semantics(
                key: AppKeys.hardwareScanProgress,
                label: localizations.hardwareScanProgressSemanticLabel,
                value: localizations.inProgressSemanticValue,
                child: const LinearProgressIndicator(),
              ),
              const SizedBox(height: 16),
              Align(
                alignment: AlignmentDirectional.centerStart,
                child: OutlinedButton.icon(
                  key: AppKeys.hardwareScanCancelAction,
                  onPressed: onCancel,
                  icon: const Icon(Icons.stop_circle_outlined),
                  label: Text(localizations.hardwareScanCancelAction),
                ),
              ),
            ] else ...[
              const SizedBox(height: 20),
              Align(
                alignment: AlignmentDirectional.centerStart,
                child: FilledButton.icon(
                  key: AppKeys.hardwareScanPrimaryAction,
                  onPressed: onStart,
                  icon: const Icon(Icons.manage_search),
                  label: Text(
                    state is HardwareScanIdle
                        ? localizations.hardwareScanStartAction
                        : localizations.hardwareScanAgainAction,
                  ),
                ),
              ),
            ],
            if (profile != null) ...[
              const SizedBox(height: 24),
              const Divider(),
              const SizedBox(height: 8),
              _MachineProfileView(profile: profile),
            ],
            if (state case HardwareScanFailed(:final diagnosticCode)) ...[
              const SizedBox(height: 12),
              ExpansionTile(
                key: AppKeys.hardwareScanDiagnostics,
                tilePadding: EdgeInsets.zero,
                childrenPadding: const EdgeInsets.only(bottom: 8),
                title: Text(localizations.diagnosticsLabel),
                children: [
                  Align(
                    alignment: AlignmentDirectional.centerStart,
                    child: SelectableText(
                      localizations.diagnosticCodeLabel(diagnosticCode),
                    ),
                  ),
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _MachineProfileView extends StatelessWidget {
  const _MachineProfileView({required this.profile});

  final MachineProfile profile;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final os = profile.operatingSystem;
    final cpu = profile.cpu;
    final memory = profile.physicalMemory;
    final storage = profile.storage;
    final rows = <Widget>[
      _EvidenceRow(
        label: localizations.hardwareOperatingSystemLabel,
        value: localizations.hardwareOperatingSystemValue(
          _text(os.name, localizations),
          _text(os.version, localizations),
          _wireValue(os.architecture.value?.wireValue, localizations),
        ),
        detail: _detail(os.name.metadata, localizations),
      ),
      _EvidenceRow(
        label: localizations.hardwareCpuLabel,
        value: localizations.hardwareCpuValue(
          _text(cpu.name, localizations),
          _number(cpu.physicalCoreCount, localizations),
          _number(cpu.logicalCoreCount, localizations),
        ),
        detail: _detail(cpu.name.metadata, localizations),
      ),
      _EvidenceRow(
        label: localizations.hardwarePhysicalMemoryLabel,
        value: localizations.hardwareMemoryValue(
          _bytes(memory.totalBytes, localizations),
          _bytes(memory.availableBytes, localizations),
        ),
        detail: _detail(memory.totalBytes.metadata, localizations),
      ),
      if (profile.gpus.devices.isEmpty)
        _EvidenceRow(
          label: localizations.hardwareGpuLabel,
          value: localizations.unknownValue,
          detail: _detail(profile.gpus.metadata, localizations),
        )
      else
        for (var index = 0; index < profile.gpus.devices.length; index += 1)
          _EvidenceRow(
            label: localizations.hardwareGpuNumberLabel(index + 1),
            value: localizations.hardwareGpuValue(
              _text(profile.gpus.devices[index].name, localizations),
              _bytes(
                profile.gpus.devices[index].dedicatedMemoryBytes,
                localizations,
              ),
              _bytes(
                profile.gpus.devices[index].sharedMemoryBytes,
                localizations,
              ),
            ),
            detail: _detail(
              profile.gpus.devices[index].dedicatedMemoryBytes.metadata,
              localizations,
            ),
          ),
      for (final acceleration in profile.acceleration)
        _EvidenceRow(
          label: localizations.hardwareAccelerationLabel,
          value: localizations.hardwareAccelerationValue(
            _wireValue(acceleration.kind.wireValue, localizations),
            switch (acceleration.supported) {
              null => localizations.unknownValue,
              true => localizations.supportedValue,
              false => localizations.notSupportedValue,
            },
          ),
          detail: _detail(acceleration.metadata, localizations),
        ),
      _EvidenceRow(
        label: localizations.hardwareStorageLabel,
        value: localizations.hardwareStorageValue(
          _bytes(storage.capacityBytes, localizations),
          _bytes(storage.freeBytes, localizations),
          _text(storage.filesystem, localizations),
          _wireValue(storage.mediaKind.value?.wireValue, localizations),
        ),
        detail: _detail(storage.capacityBytes.metadata, localizations),
      ),
    ];

    return Semantics(
      container: true,
      label: localizations.hardwareProfileSemanticLabel(
        _wireValue(profile.completeness.wireValue, localizations),
        profile.schemaVersion,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (var index = 0; index < rows.length; index += 1) ...[
            rows[index],
            if (index < rows.length - 1) const Divider(height: 24),
          ],
        ],
      ),
    );
  }

  String _text(StringEvidence evidence, AppLocalizations localizations) {
    return evidence.value ?? localizations.unknownValue;
  }

  String _number(U32Evidence evidence, AppLocalizations localizations) {
    return evidence.value?.toString() ?? localizations.unknownValue;
  }

  String _bytes(U64Evidence evidence, AppLocalizations localizations) {
    final value = evidence.value;
    if (value == null) {
      return localizations.unknownValue;
    }
    const gibibyte = 1024 * 1024 * 1024;
    if (value >= gibibyte) {
      return '${(value / gibibyte).toStringAsFixed(1)} GiB';
    }
    const mebibyte = 1024 * 1024;
    return '${(value / mebibyte).toStringAsFixed(1)} MiB';
  }

  String _wireValue(String? value, AppLocalizations localizations) {
    if (value == null) {
      return localizations.unknownValue;
    }
    return value.replaceAll('_', ' ');
  }

  String _detail(
    EvidenceMetadata metadata,
    AppLocalizations localizations,
  ) {
    return localizations.hardwareEvidenceDetail(
      _wireValue(metadata.source.wireValue, localizations),
      metadata.reason ??
          _wireValue(metadata.availability.wireValue, localizations),
    );
  }
}

class _EvidenceRow extends StatelessWidget {
  const _EvidenceRow({
    required this.label,
    required this.value,
    required this.detail,
  });

  final String label;
  final String value;
  final String detail;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: textTheme.labelLarge),
          const SizedBox(height: 4),
          Text(value, style: textTheme.bodyLarge),
          const SizedBox(height: 4),
          Text(detail, style: textTheme.bodySmall),
        ],
      ),
    );
  }
}

class _StatusPanel extends StatelessWidget {
  const _StatusPanel({
    required this.status,
    required this.state,
    required this.onRetry,
    required this.primaryActionFocusNode,
  });

  final _FoundationStatus status;
  final FoundationState state;
  final VoidCallback onRetry;
  final FocusNode? primaryActionFocusNode;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final colorScheme = Theme.of(context).colorScheme;
    final statusColor = switch (status.tone) {
      _StatusTone.ready => colorScheme.primary,
      _StatusTone.warning => colorScheme.tertiary,
      _StatusTone.error => colorScheme.error,
      _StatusTone.neutral => colorScheme.secondary,
    };
    final snapshot = switch (state) {
      FoundationReady(:final snapshot) ||
      FoundationDegraded(:final snapshot) => snapshot,
      _ => null,
    };

    return Card(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Semantics(
              key: AppKeys.foundationStatus,
              container: true,
              liveRegion: true,
              label: localizations.foundationStatusSemanticLabel(
                status.title,
                status.message,
              ),
              child: ExcludeSemantics(
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Icon(status.icon, color: statusColor, size: 32),
                    const SizedBox(width: 16),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            status.title,
                            style: Theme.of(context).textTheme.titleLarge,
                          ),
                          const SizedBox(height: 8),
                          Text(
                            status.message,
                            style: Theme.of(context).textTheme.bodyLarge,
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
            if (state is FoundationLoading) ...[
              const SizedBox(height: 24),
              Semantics(
                key: AppKeys.foundationProgress,
                label: localizations.loadingProgressSemanticLabel,
                value: localizations.inProgressSemanticValue,
                child: const LinearProgressIndicator(),
              ),
            ],
            if (snapshot != null) ...[
              const SizedBox(height: 24),
              _CoreDetails(snapshot: snapshot),
            ],
            if (state
                case FoundationFailed(:final diagnosticCode) ||
                    FoundationUnavailable(:final diagnosticCode)) ...[
              const SizedBox(height: 16),
              ExpansionTile(
                key: AppKeys.diagnostics,
                tilePadding: EdgeInsets.zero,
                childrenPadding: const EdgeInsets.only(bottom: 8),
                title: Text(localizations.diagnosticsLabel),
                children: [
                  Align(
                    alignment: AlignmentDirectional.centerStart,
                    child: SelectableText(
                      localizations.diagnosticCodeLabel(diagnosticCode),
                    ),
                  ),
                ],
              ),
            ],
            if (state is FoundationDegraded ||
                state is FoundationUnavailable ||
                state is FoundationFailed ||
                state is FoundationCancelled) ...[
              const SizedBox(height: 20),
              Align(
                alignment: AlignmentDirectional.centerStart,
                child: FilledButton.icon(
                  key: AppKeys.primaryAction,
                  focusNode: primaryActionFocusNode,
                  onPressed: onRetry,
                  icon: const Icon(Icons.refresh),
                  label: Text(localizations.checkAgainAction),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _CoreDetails extends StatelessWidget {
  const _CoreDetails({required this.snapshot});

  final CoreConnectionSnapshot snapshot;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final textTheme = Theme.of(context).textTheme;
    final readiness =
        snapshot.readiness?.wireValue ?? localizations.unknownValue;
    return Semantics(
      key: AppKeys.coreDetails,
      container: true,
      label: localizations.coreDetailsSemanticLabel(
        snapshot.applicationName ?? localizations.unknownValue,
        snapshot.coreVersion ?? localizations.unknownValue,
        snapshot.protocolVersion?.toString() ?? localizations.unknownValue,
        readiness,
      ),
      child: ExcludeSemantics(
        child: Wrap(
          spacing: 32,
          runSpacing: 16,
          children: [
            _Detail(
              label: localizations.coreApplicationLabel,
              value: snapshot.applicationName ?? localizations.unknownValue,
              textTheme: textTheme,
            ),
            _Detail(
              label: localizations.coreVersionLabel,
              value: snapshot.coreVersion ?? localizations.unknownValue,
              textTheme: textTheme,
            ),
            _Detail(
              label: localizations.protocolVersionLabel,
              value:
                  snapshot.protocolVersion?.toString() ??
                  localizations.unknownValue,
              textTheme: textTheme,
            ),
            _Detail(
              label: localizations.readinessLabel,
              value: readiness,
              textTheme: textTheme,
            ),
          ],
        ),
      ),
    );
  }
}

class _Detail extends StatelessWidget {
  const _Detail({
    required this.label,
    required this.value,
    required this.textTheme,
  });

  final String label;
  final String value;
  final TextTheme textTheme;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 160,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: textTheme.labelLarge),
          const SizedBox(height: 4),
          Text(value, style: textTheme.bodyLarge),
        ],
      ),
    );
  }
}

enum _StatusTone { ready, warning, error, neutral }

class _FoundationStatus {
  const _FoundationStatus({
    required this.title,
    required this.message,
    required this.icon,
    required this.tone,
  });

  final String title;
  final String message;
  final IconData icon;
  final _StatusTone tone;
}
