import 'package:flutter/material.dart';
import 'package:gixgiz_desktop/app/app_keys.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/features/foundation/foundation_state.dart';
import 'package:gixgiz_desktop/l10n/app_localizations.dart';

class SetupPanel extends StatelessWidget {
  const SetupPanel({
    required this.state,
    this.onApprove,
    this.onDeny,
    this.onStart,
    this.onCancel,
    this.onRetry,
    this.onRefresh,
    super.key,
  });

  final SetupWorkflowState state;
  final VoidCallback? onApprove;
  final VoidCallback? onDeny;
  final VoidCallback? onStart;
  final VoidCallback? onCancel;
  final VoidCallback? onRetry;
  final VoidCallback? onRefresh;

  @override
  Widget build(BuildContext context) {
    if (state is SetupWorkflowIdle) {
      return const SizedBox.shrink();
    }

    final localizations = AppLocalizations.of(context);
    final job = _jobFor(state);
    final status = _statusFor(localizations, state);
    final statusColor = switch (status.tone) {
      _SetupTone.ready => Theme.of(context).colorScheme.primary,
      _SetupTone.warning => Theme.of(context).colorScheme.tertiary,
      _SetupTone.error => Theme.of(context).colorScheme.error,
      _SetupTone.neutral => Theme.of(context).colorScheme.secondary,
    };
    final diagnosticCode = switch (state) {
      SetupWorkflowFailed(:final diagnosticCode) => diagnosticCode,
      _ => job?.error?.code,
    };
    final transportRecoveryAction = switch (state) {
      SetupWorkflowFailed(:final transportRecoveryAction) =>
        transportRecoveryAction,
      _ => null,
    };
    final transportRecoveryMessage = switch (state) {
      SetupWorkflowFailed(:final transportRecoveryMessage) =>
        transportRecoveryMessage,
      _ => null,
    };
    final safeErrorRecoveryAction =
        transportRecoveryAction ?? job?.error?.recovery.action;
    final safeErrorRecoveryMessage =
        transportRecoveryMessage ?? job?.error?.recovery.message;

    return Card(
      key: AppKeys.setupPanel,
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              localizations.setupTitle,
              style: Theme.of(context).textTheme.titleLarge,
            ),
            const SizedBox(height: 16),
            Semantics(
              key: AppKeys.setupStatus,
              container: true,
              liveRegion: true,
              label: localizations.setupStatusSemanticLabel(
                status.title,
                status.message,
                job?.plan.model.displayName ?? localizations.unknownValue,
                job == null
                    ? localizations.unknownValue
                    : _stageLabel(localizations, job.stage),
              ),
              child: ExcludeSemantics(
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Icon(status.icon, color: statusColor, size: 28),
                    const SizedBox(width: 16),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            status.title,
                            style: Theme.of(context).textTheme.titleMedium,
                          ),
                          const SizedBox(height: 6),
                          Text(status.message),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
            if (state is SetupWorkflowPlanning ||
                state is SetupWorkflowLoading ||
                state is SetupWorkflowActive) ...[
              const SizedBox(height: 20),
              _SetupProgress(job: job),
            ],
            if (job != null) ...[
              const SizedBox(height: 20),
              const Divider(),
              const SizedBox(height: 12),
              _SetupPlanDetails(job: job),
              if (job.attentionReason case final reason?) ...[
                const SizedBox(height: 16),
                _Notice(
                  icon: Icons.warning_amber,
                  message: _attentionLabel(localizations, reason),
                ),
              ],
              if (safeErrorRecoveryAction == null &&
                  job.recoveryAction != null) ...[
                const SizedBox(height: 12),
                _Notice(
                  icon: Icons.tips_and_updates_outlined,
                  message: localizations.setupRecommendedAction(
                    _recoveryLabel(localizations, job.recoveryAction!),
                  ),
                ),
              ],
              if (_effectReport(job) case final report?) ...[
                const SizedBox(height: 20),
                const Divider(),
                const SizedBox(height: 12),
                _SetupEffects(report: report),
              ],
            ],
            if (safeErrorRecoveryAction != null) ...[
              const SizedBox(height: 12),
              _Notice(
                icon: Icons.tips_and_updates_outlined,
                message: localizations.setupRecommendedAction(
                  _transportRecoveryLabel(
                    localizations,
                    safeErrorRecoveryAction,
                  ),
                ),
              ),
            ],
            const SizedBox(height: 20),
            _SetupActions(
              state: state,
              job: job,
              onApprove: onApprove,
              onDeny: onDeny,
              onStart: onStart,
              onCancel: onCancel,
              onRetry: onRetry,
              onRefresh: onRefresh,
            ),
            if (diagnosticCode != null) ...[
              const SizedBox(height: 12),
              ExpansionTile(
                key: AppKeys.setupDiagnostics,
                tilePadding: EdgeInsets.zero,
                childrenPadding: const EdgeInsets.only(bottom: 8),
                title: Text(localizations.diagnosticsLabel),
                children: [
                  Align(
                    alignment: AlignmentDirectional.centerStart,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        SelectableText(
                          localizations.diagnosticCodeLabel(diagnosticCode),
                        ),
                        if (safeErrorRecoveryMessage != null) ...[
                          const SizedBox(height: 8),
                          SelectableText(safeErrorRecoveryMessage),
                        ],
                      ],
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

class _SetupProgress extends StatelessWidget {
  const _SetupProgress({required this.job});

  final SetupJobSnapshot? job;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final progress = job?.progress;
    final basisPoints = progress?.progressBasisPoints;
    final value = basisPoints == null
        ? null
        : (basisPoints.clamp(0, 10000) / 10000).toDouble();
    final semanticValue = switch (progress) {
      ModelAcquisitionProgress(:final completedBytes?, :final totalBytes?)
          when totalBytes > 0 =>
        localizations.setupProgressBytes(
          _formatBytes(completedBytes),
          _formatBytes(totalBytes),
        ),
      _ when basisPoints != null => localizations.setupProgressPercent(
        (basisPoints / 100).toStringAsFixed(basisPoints % 100 == 0 ? 0 : 2),
      ),
      _ => localizations.inProgressSemanticValue,
    };
    return Semantics(
      key: AppKeys.setupProgress,
      label: localizations.setupProgressSemanticLabel,
      value: semanticValue,
      child: LinearProgressIndicator(value: value),
    );
  }
}

class _SetupPlanDetails extends StatelessWidget {
  const _SetupPlanDetails({required this.job});

  final SetupJobSnapshot job;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final plan = job.plan;
    final model = plan.model;
    final textTheme = Theme.of(context).textTheme;
    return Column(
      key: AppKeys.setupPlan,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Wrap(
          spacing: 28,
          runSpacing: 16,
          children: [
            _Detail(
              label: localizations.setupModelLabel,
              value: model.displayName,
            ),
            _Detail(label: localizations.setupFamilyLabel, value: model.family),
            _Detail(
              label: localizations.setupProviderArtifactLabel,
              value: model.artifact.providerModelId,
            ),
            _Detail(
              label: localizations.setupRuntimeLabel,
              value: plan.runtimeDisplayName,
            ),
            _Detail(
              label: localizations.setupRuntimeVersionLabel,
              value: plan.runtimeVersion ?? localizations.unknownValue,
            ),
            _Detail(
              label: localizations.setupLicenceLabel,
              value: model.licenceSpdx,
            ),
            _Detail(
              label: localizations.setupDestinationLabel,
              value: model.destinationDisplay,
            ),
            _Detail(
              label: localizations.setupExpectedSizeLabel,
              value: _formatBytes(model.expectedSizeBytes),
            ),
            _Detail(
              label: localizations.capabilityMemoryLabel,
              value: _formatBytes(
                plan.resources.memory.requiredBytes +
                    plan.resources.memory.safetyMarginBytes,
              ),
            ),
            _Detail(
              label: localizations.capabilityStorageEstimateLabel,
              value: _formatBytes(
                plan.resources.storage.requiredBytes +
                    plan.resources.storage.safetyMarginBytes,
              ),
            ),
          ],
        ),
        const SizedBox(height: 16),
        Text(localizations.setupProvenanceLabel, style: textTheme.labelLarge),
        const SizedBox(height: 4),
        SelectableText(model.provenance),
        if (plan.components.isNotEmpty) ...[
          const SizedBox(height: 16),
          _MessageList(
            title: localizations.setupPlanComponentsLabel,
            messages: plan.components.map((component) {
              final label = _componentKindLabel(localizations, component.kind);
              return component.required
                  ? localizations.setupComponentRequired(label)
                  : localizations.setupComponentOptional(label);
            }),
            icon: Icons.check_circle_outline,
          ),
        ],
        if (plan.requiredEffects.isNotEmpty) ...[
          const SizedBox(height: 16),
          _MessageList(
            title: localizations.setupRequiredEffectsLabel,
            messages: plan.requiredEffects.map(
              (effect) => _effectKindLabel(localizations, effect),
            ),
            icon: Icons.fact_check_outlined,
          ),
        ],
        if (plan.reasons.isNotEmpty) ...[
          const SizedBox(height: 16),
          _MessageList(
            title: localizations.setupReasonsLabel,
            messages: plan.reasons.map(
              (reason) => _reasonLabel(localizations, reason.code),
            ),
            icon: Icons.info_outline,
          ),
        ],
        if (plan.warnings.isNotEmpty) ...[
          const SizedBox(height: 12),
          _MessageList(
            title: localizations.setupWarningsLabel,
            messages: plan.warnings.map(
              (warning) => _warningLabel(localizations, warning.code),
            ),
            icon: Icons.warning_amber,
          ),
        ],
      ],
    );
  }
}

class _SetupActions extends StatelessWidget {
  const _SetupActions({
    required this.state,
    required this.job,
    required this.onApprove,
    required this.onDeny,
    required this.onStart,
    required this.onCancel,
    required this.onRetry,
    required this.onRefresh,
  });

  final SetupWorkflowState state;
  final SetupJobSnapshot? job;
  final VoidCallback? onApprove;
  final VoidCallback? onDeny;
  final VoidCallback? onStart;
  final VoidCallback? onCancel;
  final VoidCallback? onRetry;
  final VoidCallback? onRefresh;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final busy =
        state is SetupWorkflowPlanning || state is SetupWorkflowLoading;
    return Wrap(
      spacing: 12,
      runSpacing: 12,
      children: [
        if (state is SetupWorkflowAwaitingApproval && job != null) ...[
          FilledButton.icon(
            key: AppKeys.setupApproveAction,
            onPressed: onApprove == null
                ? null
                : () => _confirmApproval(context, job!),
            icon: const Icon(Icons.check_circle_outline),
            label: Text(localizations.setupApproveAction),
          ),
          TextButton(
            key: AppKeys.setupDenyAction,
            onPressed: onDeny,
            child: Text(localizations.setupDenyAction),
          ),
        ],
        if (state is SetupWorkflowApproved)
          FilledButton.icon(
            key: AppKeys.setupStartAction,
            onPressed: onStart,
            icon: const Icon(Icons.play_arrow),
            label: Text(localizations.setupStartAction),
          ),
        if (state is SetupWorkflowApproved ||
            state is SetupWorkflowActive ||
            state is SetupWorkflowAttention)
          OutlinedButton.icon(
            key: AppKeys.setupCancelAction,
            onPressed: job?.cancellationRequested == true || onCancel == null
                ? null
                : () => _confirmCancellation(context, state),
            icon: const Icon(Icons.stop_circle_outlined),
            label: Text(localizations.setupCancelAction),
          ),
        if (onRetry != null)
          FilledButton.icon(
            key: AppKeys.setupRetryAction,
            onPressed: busy ? null : onRetry,
            icon: const Icon(Icons.refresh),
            label: Text(localizations.setupRetryAction),
          ),
        if (onRefresh != null)
          OutlinedButton.icon(
            key: AppKeys.setupRefreshAction,
            onPressed: busy ? null : onRefresh,
            icon: const Icon(Icons.sync),
            label: Text(localizations.setupRefreshAction),
          ),
      ],
    );
  }

  Future<void> _confirmApproval(
    BuildContext context,
    SetupJobSnapshot job,
  ) async {
    final localizations = AppLocalizations.of(context);
    final plan = job.plan;
    final approved = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(localizations.setupApprovalPromptTitle),
        content: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                localizations.setupApprovalPromptMessage(
                  plan.model.displayName,
                  plan.runtimeDisplayName,
                  _formatBytes(plan.model.expectedSizeBytes),
                  plan.model.destinationDisplay,
                ),
              ),
              const SizedBox(height: 16),
              for (final effect in plan.requiredEffects)
                Padding(
                  padding: const EdgeInsets.only(bottom: 8),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const Icon(Icons.check, size: 18),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Text(_effectKindLabel(localizations, effect)),
                      ),
                    ],
                  ),
                ),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(localizations.setupApprovalBackAction),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(localizations.setupApproveAction),
          ),
        ],
      ),
    );
    if (approved == true) {
      onApprove?.call();
    }
  }

  Future<void> _confirmCancellation(
    BuildContext context,
    SetupWorkflowState state,
  ) async {
    final localizations = AppLocalizations.of(context);
    final message = switch (state) {
      SetupWorkflowApproved() => localizations.setupCancelApprovedPromptMessage,
      SetupWorkflowAttention() =>
        localizations.setupCancelAttentionPromptMessage,
      _ => localizations.setupCancelPromptMessage,
    };
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(localizations.setupCancelPromptTitle),
        content: Text(message),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(localizations.setupKeepRunningAction),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(localizations.setupCancelAction),
          ),
        ],
      ),
    );
    if (confirmed == true) {
      onCancel?.call();
    }
  }
}

class _SetupEffects extends StatelessWidget {
  const _SetupEffects({required this.report});

  final SetupEffectReport report;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    return Column(
      key: AppKeys.setupEffects,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          localizations.setupEffectsTitle,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 10),
        for (final effect in report.effects)
          Padding(
            padding: const EdgeInsets.only(bottom: 10),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Icon(_effectIcon(effect.disposition), size: 20),
                const SizedBox(width: 10),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        '${_effectDispositionLabel(localizations, effect.disposition)}: '
                        '${_effectKindLabel(localizations, effect.kind)}',
                        style: Theme.of(context).textTheme.labelLarge,
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
      ],
    );
  }
}

class _MessageList extends StatelessWidget {
  const _MessageList({
    required this.title,
    required this.messages,
    required this.icon,
  });

  final String title;
  final Iterable<String> messages;
  final IconData icon;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(title, style: Theme.of(context).textTheme.labelLarge),
        const SizedBox(height: 6),
        for (final message in messages)
          Padding(
            padding: const EdgeInsets.only(bottom: 6),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Icon(icon, size: 18),
                const SizedBox(width: 8),
                Expanded(child: Text(message)),
              ],
            ),
          ),
      ],
    );
  }
}

class _Detail extends StatelessWidget {
  const _Detail({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 180,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: Theme.of(context).textTheme.labelLarge),
          const SizedBox(height: 4),
          Text(value),
        ],
      ),
    );
  }
}

class _Notice extends StatelessWidget {
  const _Notice({required this.icon, required this.message});

  final IconData icon;
  final String message;

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Icon(icon, size: 20),
        const SizedBox(width: 8),
        Expanded(child: Text(message)),
      ],
    );
  }
}

SetupJobSnapshot? _jobFor(SetupWorkflowState state) {
  return switch (state) {
    SetupWorkflowLoading(:final previousJob) => previousJob,
    SetupWorkflowAwaitingApproval(:final job) ||
    SetupWorkflowApproved(:final job) ||
    SetupWorkflowActive(:final job) ||
    SetupWorkflowReady(:final job) ||
    SetupWorkflowAttention(:final job) ||
    SetupWorkflowCancelled(:final job) ||
    SetupWorkflowUnknown(:final job) => job,
    SetupWorkflowFailed(:final job) => job,
    SetupWorkflowIdle() || SetupWorkflowPlanning() => null,
  };
}

SetupEffectReport? _effectReport(SetupJobSnapshot job) {
  return job.cancellationReport?.effectReport ?? job.effectReport;
}

_SetupStatus _statusFor(
  AppLocalizations localizations,
  SetupWorkflowState state,
) {
  return switch (state) {
    SetupWorkflowIdle() => _SetupStatus(
      title: localizations.setupTitle,
      message: localizations.setupPlanReadyMessage,
      icon: Icons.inventory_2_outlined,
      tone: _SetupTone.neutral,
    ),
    SetupWorkflowPlanning() => _SetupStatus(
      title: localizations.setupPlanningTitle,
      message: localizations.setupPlanningMessage,
      icon: Icons.sync,
      tone: _SetupTone.neutral,
    ),
    SetupWorkflowLoading() => _SetupStatus(
      title: localizations.setupUpdatingTitle,
      message: localizations.setupUpdatingMessage,
      icon: Icons.sync,
      tone: _SetupTone.neutral,
    ),
    SetupWorkflowAwaitingApproval() => _SetupStatus(
      title: localizations.setupAwaitingApprovalTitle,
      message: localizations.setupAwaitingApprovalMessage,
      icon: Icons.fact_check_outlined,
      tone: _SetupTone.warning,
    ),
    SetupWorkflowApproved() => _SetupStatus(
      title: localizations.setupApprovedTitle,
      message: localizations.setupApprovedMessage,
      icon: Icons.check_circle_outline,
      tone: _SetupTone.ready,
    ),
    SetupWorkflowActive(:final job) when job.cancellationRequested =>
      _SetupStatus(
        title: localizations.setupCancellingTitle,
        message: localizations.setupCancellingMessage,
        icon: Icons.stop_circle_outlined,
        tone: _SetupTone.warning,
      ),
    SetupWorkflowActive(:final job) => _activeStatus(localizations, job.stage),
    SetupWorkflowReady() => _SetupStatus(
      title: localizations.setupReadyTitle,
      message: localizations.setupReadyMessage,
      icon: Icons.check_circle_outline,
      tone: _SetupTone.ready,
    ),
    SetupWorkflowAttention() => _SetupStatus(
      title: localizations.setupAttentionTitle,
      message: localizations.setupAttentionMessage,
      icon: Icons.warning_amber,
      tone: _SetupTone.warning,
    ),
    SetupWorkflowFailed() => _SetupStatus(
      title: localizations.setupFailedTitle,
      message: localizations.setupFailedMessage,
      icon: Icons.error_outline,
      tone: _SetupTone.error,
    ),
    SetupWorkflowCancelled() => _SetupStatus(
      title: localizations.setupCancelledTitle,
      message: localizations.setupCancelledMessage,
      icon: Icons.cancel_outlined,
      tone: _SetupTone.neutral,
    ),
    SetupWorkflowUnknown() => _SetupStatus(
      title: localizations.setupUnknownTitle,
      message: localizations.setupUnknownMessage,
      icon: Icons.help_outline,
      tone: _SetupTone.warning,
    ),
  };
}

_SetupStatus _activeStatus(AppLocalizations localizations, SetupStage stage) {
  return switch (stage) {
    SetupStage.preparing => _SetupStatus(
      title: localizations.setupPreparingTitle,
      message: localizations.setupPreparingMessage,
      icon: Icons.sync,
      tone: _SetupTone.neutral,
    ),
    SetupStage.checkingStorage => _SetupStatus(
      title: localizations.setupCheckingStorageTitle,
      message: localizations.setupCheckingStorageMessage,
      icon: Icons.storage_outlined,
      tone: _SetupTone.neutral,
    ),
    SetupStage.acquiring => _SetupStatus(
      title: localizations.setupAcquiringTitle,
      message: localizations.setupAcquiringMessage,
      icon: Icons.download_outlined,
      tone: _SetupTone.neutral,
    ),
    SetupStage.registering => _SetupStatus(
      title: localizations.setupRegisteringTitle,
      message: localizations.setupRegisteringMessage,
      icon: Icons.inventory_2_outlined,
      tone: _SetupTone.neutral,
    ),
    SetupStage.verifyingRuntime => _SetupStatus(
      title: localizations.setupVerifyingRuntimeTitle,
      message: localizations.setupVerifyingRuntimeMessage,
      icon: Icons.health_and_safety_outlined,
      tone: _SetupTone.neutral,
    ),
    SetupStage.verifyingModel => _SetupStatus(
      title: localizations.setupVerifyingModelTitle,
      message: localizations.setupVerifyingModelMessage,
      icon: Icons.verified_outlined,
      tone: _SetupTone.neutral,
    ),
    SetupStage.runningTestInference => _SetupStatus(
      title: localizations.setupInferenceTitle,
      message: localizations.setupInferenceMessage,
      icon: Icons.fact_check_outlined,
      tone: _SetupTone.neutral,
    ),
    _ => _SetupStatus(
      title: localizations.setupPreparingTitle,
      message: localizations.setupPreparingMessage,
      icon: Icons.sync,
      tone: _SetupTone.neutral,
    ),
  };
}

String _stageLabel(AppLocalizations localizations, SetupStage stage) {
  return switch (stage) {
    SetupStage.draftPlan => localizations.setupPlanReadyTitle,
    SetupStage.awaitingApproval => localizations.setupAwaitingApprovalTitle,
    SetupStage.approved => localizations.setupApprovedTitle,
    SetupStage.preparing => localizations.setupPreparingTitle,
    SetupStage.checkingStorage => localizations.setupCheckingStorageTitle,
    SetupStage.acquiring => localizations.setupAcquiringTitle,
    SetupStage.registering => localizations.setupRegisteringTitle,
    SetupStage.verifyingRuntime => localizations.setupVerifyingRuntimeTitle,
    SetupStage.verifyingModel => localizations.setupVerifyingModelTitle,
    SetupStage.runningTestInference => localizations.setupInferenceTitle,
    SetupStage.ready => localizations.setupReadyTitle,
    SetupStage.attentionRequired => localizations.setupAttentionTitle,
    SetupStage.failed => localizations.setupFailedTitle,
    SetupStage.cancelled => localizations.setupCancelledTitle,
    SetupStage.unknown => localizations.setupUnknownTitle,
  };
}

String _attentionLabel(
  AppLocalizations localizations,
  SetupAttentionReason reason,
) {
  return switch (reason) {
    SetupAttentionReason.runtimeNotInstalled =>
      localizations.setupAttentionRuntimeNotInstalled,
    SetupAttentionReason.runtimeConsentRequired =>
      localizations.setupAttentionRuntimeConsentRequired,
    SetupAttentionReason.runtimeUnavailable =>
      localizations.setupAttentionRuntimeUnavailable,
    SetupAttentionReason.privilegedRuntimeInstallationRequired =>
      localizations.setupAttentionPrivilegedRuntime,
    SetupAttentionReason.destinationUnavailable =>
      localizations.setupAttentionDestinationUnavailable,
    SetupAttentionReason.insufficientStorage =>
      localizations.setupAttentionInsufficientStorage,
    SetupAttentionReason.acquisitionInterrupted =>
      localizations.setupAttentionAcquisitionInterrupted,
    SetupAttentionReason.registrationUnverified =>
      localizations.setupAttentionRegistrationUnverified,
    SetupAttentionReason.modelUnavailable =>
      localizations.setupAttentionModelUnavailable,
    SetupAttentionReason.integrityMismatch =>
      localizations.setupAttentionIntegrityMismatch,
    SetupAttentionReason.readinessTimedOut =>
      localizations.setupAttentionReadinessTimedOut,
    SetupAttentionReason.readinessFailed =>
      localizations.setupAttentionReadinessFailed,
    SetupAttentionReason.recoveryRequired =>
      localizations.setupAttentionRecoveryRequired,
    SetupAttentionReason.unknown => localizations.setupAttentionUnknown,
  };
}

String _recoveryLabel(
  AppLocalizations localizations,
  SetupRecoveryAction action,
) {
  return switch (action) {
    SetupRecoveryAction.reviewApproval =>
      localizations.setupRecoveryReviewApproval,
    SetupRecoveryAction.retry => localizations.setupRecoveryRetry,
    SetupRecoveryAction.restoreDestination =>
      localizations.setupRecoveryRestoreDestination,
    SetupRecoveryAction.freeStorage => localizations.setupRecoveryFreeStorage,
    SetupRecoveryAction.restoreRuntime =>
      localizations.setupRecoveryRestoreRuntime,
    SetupRecoveryAction.checkPrerequisites =>
      localizations.setupRecoveryCheckPrerequisites,
    SetupRecoveryAction.contactSupport =>
      localizations.setupRecoveryContactSupport,
    SetupRecoveryAction.noAction => localizations.setupRecoveryNoAction,
    SetupRecoveryAction.unknown => localizations.setupRecoveryUnknown,
  };
}

String _componentKindLabel(
  AppLocalizations localizations,
  SetupPlanComponentKind kind,
) {
  return switch (kind) {
    SetupPlanComponentKind.runtime => localizations.setupComponentRuntime,
    SetupPlanComponentKind.model => localizations.setupComponentModel,
    SetupPlanComponentKind.storage => localizations.setupComponentStorage,
    SetupPlanComponentKind.verification =>
      localizations.setupComponentVerification,
    SetupPlanComponentKind.unknown => localizations.setupComponentUnknown,
  };
}

String _transportRecoveryLabel(
  AppLocalizations localizations,
  RecoveryAction action,
) {
  return switch (action) {
    RecoveryAction.retry => localizations.runtimeRecoveryRetry,
    RecoveryAction.restart => localizations.runtimeRecoveryRestart,
    RecoveryAction.checkPrerequisites =>
      localizations.runtimeRecoveryCheckPrerequisites,
    RecoveryAction.contactSupport =>
      localizations.runtimeRecoveryContactSupport,
    RecoveryAction.noAction => localizations.runtimeRecoveryNoAction,
    RecoveryAction.unknown => localizations.runtimeRecoveryUnknown,
  };
}

String _reasonLabel(AppLocalizations localizations, SetupReasonCode code) {
  return switch (code) {
    SetupReasonCode.recommendationSelected =>
      localizations.setupReasonRecommendationSelected,
    SetupReasonCode.existingModelReusable =>
      localizations.setupReasonExistingModelReusable,
    SetupReasonCode.acquisitionRequired =>
      localizations.setupReasonAcquisitionRequired,
    SetupReasonCode.approvalRequired =>
      localizations.setupReasonApprovalRequired,
    SetupReasonCode.storageVerified => localizations.setupReasonStorageVerified,
    SetupReasonCode.runtimeVerified => localizations.setupReasonRuntimeVerified,
    SetupReasonCode.registrationVerified =>
      localizations.setupReasonRegistrationVerified,
    SetupReasonCode.readinessVerified =>
      localizations.setupReasonReadinessVerified,
    SetupReasonCode.unknown => localizations.setupReasonUnknown,
  };
}

String _warningLabel(AppLocalizations localizations, SetupWarningCode code) {
  return switch (code) {
    SetupWarningCode.externalRuntimeModified =>
      localizations.setupWarningExternalRuntimeModified,
    SetupWarningCode.providerManagedStorage =>
      localizations.setupWarningProviderManagedStorage,
    SetupWarningCode.integrityMetadataUnavailable =>
      localizations.setupWarningIntegrityMetadataUnavailable,
    SetupWarningCode.cancellationMayRetainEffects =>
      localizations.setupWarningCancellationMayRetainEffects,
    SetupWarningCode.destinationEvidenceIncomplete =>
      localizations.setupWarningDestinationEvidenceIncomplete,
    SetupWarningCode.unknown => localizations.setupWarningUnknown,
  };
}

String _effectKindLabel(AppLocalizations localizations, SetupEffectKind kind) {
  return switch (kind) {
    SetupEffectKind.providerModelAcquisition =>
      localizations.setupEffectProviderAcquisition,
    SetupEffectKind.providerModelRegistration =>
      localizations.setupEffectProviderRegistration,
    SetupEffectKind.readinessInference =>
      localizations.setupEffectReadinessInference,
    SetupEffectKind.metadataPersistence =>
      localizations.setupEffectMetadataPersistence,
    SetupEffectKind.unknown => localizations.setupEffectUnknown,
  };
}

String _effectDispositionLabel(
  AppLocalizations localizations,
  SetupEffectDisposition disposition,
) {
  return switch (disposition) {
    SetupEffectDisposition.completed => localizations.setupEffectCompleted,
    SetupEffectDisposition.retained => localizations.setupEffectRetained,
    SetupEffectDisposition.rolledBack => localizations.setupEffectRolledBack,
    SetupEffectDisposition.uncertain => localizations.setupEffectUncertain,
    SetupEffectDisposition.unknown =>
      localizations.setupEffectDispositionUnknown,
  };
}

IconData _effectIcon(SetupEffectDisposition disposition) {
  return switch (disposition) {
    SetupEffectDisposition.completed => Icons.check_circle_outline,
    SetupEffectDisposition.retained => Icons.inventory_2_outlined,
    SetupEffectDisposition.rolledBack => Icons.undo,
    SetupEffectDisposition.uncertain ||
    SetupEffectDisposition.unknown => Icons.help_outline,
  };
}

String _formatBytes(int bytes) {
  const gibibyte = 1024 * 1024 * 1024;
  if (bytes >= gibibyte) {
    return '${(bytes / gibibyte).toStringAsFixed(1)} GiB';
  }
  const mebibyte = 1024 * 1024;
  if (bytes >= mebibyte) {
    return '${(bytes / mebibyte).toStringAsFixed(1)} MiB';
  }
  const kibibyte = 1024;
  return '${(bytes / kibibyte).toStringAsFixed(1)} KiB';
}

enum _SetupTone { ready, warning, error, neutral }

class _SetupStatus {
  const _SetupStatus({
    required this.title,
    required this.message,
    required this.icon,
    required this.tone,
  });

  final String title;
  final String message;
  final IconData icon;
  final _SetupTone tone;
}
