import 'package:flutter/widgets.dart';

abstract final class AppKeys {
  static const brand = ValueKey<String>('app.brand');
  static const foundationNavigation = ValueKey<String>('navigation.foundation');
  static const aboutNavigation = ValueKey<String>('navigation.about');
  static const foundationStatus = ValueKey<String>('foundation.status');
  static const foundationProgress = ValueKey<String>('foundation.progress');
  static const primaryAction = ValueKey<String>('foundation.primary_action');
  static const diagnostics = ValueKey<String>('foundation.diagnostics');
  static const coreDetails = ValueKey<String>('foundation.core_details');
  static const hardwareScan = ValueKey<String>('foundation.hardware_scan');
  static const hardwareScanStatus = ValueKey<String>(
    'foundation.hardware_scan.status',
  );
  static const hardwareScanProgress = ValueKey<String>(
    'foundation.hardware_scan.progress',
  );
  static const hardwareScanPrimaryAction = ValueKey<String>(
    'foundation.hardware_scan.primary_action',
  );
  static const hardwareScanCancelAction = ValueKey<String>(
    'foundation.hardware_scan.cancel_action',
  );
  static const hardwareScanDiagnostics = ValueKey<String>(
    'foundation.hardware_scan.diagnostics',
  );
  static const capabilityRecommendation = ValueKey<String>(
    'foundation.capability',
  );
  static const capabilityWorkload = ValueKey<String>(
    'foundation.capability.workload',
  );
  static const capabilityLargerOption = ValueKey<String>(
    'foundation.capability.larger_option',
  );
  static const capabilityGenerateAction = ValueKey<String>(
    'foundation.capability.generate_action',
  );
  static const capabilityProgress = ValueKey<String>(
    'foundation.capability.progress',
  );
  static const capabilityStatus = ValueKey<String>(
    'foundation.capability.status',
  );
  static const capabilityRecommendedPlan = ValueKey<String>(
    'foundation.capability.recommended',
  );
  static const capabilityFallbackPlan = ValueKey<String>(
    'foundation.capability.fallback',
  );
  static const capabilityLargerPlan = ValueKey<String>(
    'foundation.capability.larger',
  );
  static const capabilityNoPlan = ValueKey<String>(
    'foundation.capability.no_plan',
  );
  static const setupPanel = ValueKey<String>('foundation.setup');
  static const setupStatus = ValueKey<String>('foundation.setup.status');
  static const setupProgress = ValueKey<String>('foundation.setup.progress');
  static const setupPlan = ValueKey<String>('foundation.setup.plan');
  static const setupReviewAction = ValueKey<String>(
    'foundation.setup.review_action',
  );
  static const setupApproveAction = ValueKey<String>(
    'foundation.setup.approve_action',
  );
  static const setupDenyAction = ValueKey<String>(
    'foundation.setup.deny_action',
  );
  static const setupStartAction = ValueKey<String>(
    'foundation.setup.start_action',
  );
  static const setupCancelAction = ValueKey<String>(
    'foundation.setup.cancel_action',
  );
  static const setupRetryAction = ValueKey<String>(
    'foundation.setup.retry_action',
  );
  static const setupRefreshAction = ValueKey<String>(
    'foundation.setup.refresh_action',
  );
  static const setupEffects = ValueKey<String>('foundation.setup.effects');
  static const setupDiagnostics = ValueKey<String>(
    'foundation.setup.diagnostics',
  );
  static const runtimePanel = ValueKey<String>('foundation.runtime');
  static const runtimeStatus = ValueKey<String>('foundation.runtime.status');
  static const runtimeDetails = ValueKey<String>('foundation.runtime.details');
  static const runtimeProgress = ValueKey<String>(
    'foundation.runtime.progress',
  );
  static const runtimeRefreshAction = ValueKey<String>(
    'foundation.runtime.refresh_action',
  );
  static const runtimeConsentAction = ValueKey<String>(
    'foundation.runtime.consent_action',
  );
  static const runtimeStartAction = ValueKey<String>(
    'foundation.runtime.start_action',
  );
  static const runtimeStopAction = ValueKey<String>(
    'foundation.runtime.stop_action',
  );
  static const runtimeRestartAction = ValueKey<String>(
    'foundation.runtime.restart_action',
  );
  static const runtimeCancelAction = ValueKey<String>(
    'foundation.runtime.cancel_action',
  );
  static const runtimeModelsAction = ValueKey<String>(
    'foundation.runtime.models_action',
  );
  static const runtimeModels = ValueKey<String>('foundation.runtime.models');
  static const runtimeModelsProgress = ValueKey<String>(
    'foundation.runtime.models_progress',
  );
  static const runtimeDiagnostics = ValueKey<String>(
    'foundation.runtime.diagnostics',
  );
  static const chatNavigation = ValueKey<String>('navigation.chat');
  static const chatLocalityBadge = ValueKey<String>('chat.locality');
  static const chatNewConversationButton = ValueKey<String>('chat.new');
  static const chatConversationList = ValueKey<String>('chat.conversations');
  static const chatConversationsEmpty = ValueKey<String>('chat.conversations_empty');
  static const chatNoConversationSelected = ValueKey<String>('chat.unselected');
  static const chatMessageList = ValueKey<String>('chat.messages');
  static const chatStreamingMessage = ValueKey<String>('chat.streaming');
  static const chatComposer = ValueKey<String>('chat.composer');
  static const chatSendButton = ValueKey<String>('chat.send');
  static const chatStopButton = ValueKey<String>('chat.stop');
  static const chatRenameButton = ValueKey<String>('chat.rename');
  static const chatDeleteButton = ValueKey<String>('chat.delete');
  static const chatDeleteConfirmButton = ValueKey<String>('chat.delete_confirm');
  static const chatRetryButton = ValueKey<String>('chat.retry');
  static const chatAttention = ValueKey<String>('chat.attention');
  static const chatFailed = ValueKey<String>('chat.failed');
}
