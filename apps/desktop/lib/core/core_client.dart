import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';

enum CoreConnectionKind { ready, degraded, unavailable, failed, cancelled }

enum CoreConnectionIssue {
  none,
  missingCore,
  startupTimedOut,
  protocolMismatch,
  authenticationFailed,
  connectionLost,
  coreUnavailable,
  cancelled,
  internalFailure,
}

class CoreConnectionSnapshot {
  const CoreConnectionSnapshot({
    required this.kind,
    this.issue = CoreConnectionIssue.none,
    this.diagnosticCode,
    this.applicationName,
    this.coreVersion,
    this.protocolVersion,
    this.readiness,
    this.readinessSummary,
  });

  final CoreConnectionKind kind;
  final CoreConnectionIssue issue;
  final String? diagnosticCode;
  final String? applicationName;
  final String? coreVersion;
  final int? protocolVersion;
  final ReadinessStatus? readiness;
  final String? readinessSummary;
}

class CoreOperation {
  const CoreOperation({required this.operationId, required this.correlationId});

  final OperationId operationId;
  final CorrelationId correlationId;
}

class CoreClientFailure implements Exception {
  const CoreClientFailure({
    required this.code,
    required this.category,
    required this.recoveryAction,
    this.recoveryMessage,
  });

  final String code;
  final ErrorCategory category;
  final RecoveryAction recoveryAction;
  final String? recoveryMessage;

  @override
  String toString() => 'CoreClientFailure($category, $code)';
}

abstract class CoreClient {
  const CoreClient();

  bool supportsTransportCapability(TransportCapability capability) => false;

  Future<CoreConnectionSnapshot> checkConnection();

  Future<CoreOperation> startFoundationOperation() {
    return Future.error(
      UnsupportedError(
        'Foundation operations are not supported by this client.',
      ),
    );
  }

  Stream<TestOperationEvent> observeFoundationOperation(
    CoreOperation operation,
  ) {
    return Stream.error(
      UnsupportedError(
        'Foundation event streams are not supported by this client.',
      ),
    );
  }

  Future<bool> cancelFoundationOperation(CoreOperation operation) {
    return Future.error(
      UnsupportedError(
        'Foundation cancellation is not supported by this client.',
      ),
    );
  }

  Future<CoreOperation> startHardwareScan() {
    return Future.error(
      UnsupportedError('Hardware scans are not supported by this client.'),
    );
  }

  Stream<HardwareScanEvent> observeHardwareScan(CoreOperation operation) {
    return Stream.error(
      UnsupportedError(
        'Hardware scan event streams are not supported by this client.',
      ),
    );
  }

  Future<bool> cancelHardwareScan(CoreOperation operation) {
    return Future.error(
      UnsupportedError('Hardware scan cancellation is not supported.'),
    );
  }

  Future<CapabilityReport> recommendCapability(
    MachineProfile profile,
    UserPreferenceProfile preferences,
  ) {
    return Future.error(
      UnsupportedError(
        'Capability recommendations are not supported by this client.',
      ),
    );
  }

  Future<RuntimeHealthReport> checkRuntimeStatus() {
    return Future.error(
      UnsupportedError('Runtime status is not supported by this client.'),
    );
  }

  Future<RuntimeHealthReport> decideRuntimeReuse(
    RuntimeConsentDecision decision,
  ) {
    return Future.error(
      UnsupportedError('Runtime consent is not supported by this client.'),
    );
  }

  /// Accepts one exact runtime version that is outside recorded support evidence.
  ///
  /// The acknowledgement is bound to that exact version by the core; a later
  /// provider version is untested again and requires a new decision.
  Future<RuntimeHealthReport> acknowledgeUntestedRuntimeVersion(String version) {
    return Future.error(
      UnsupportedError('Runtime consent is not supported by this client.'),
    );
  }

  /// Builds a reviewable local-runtime installation plan.
  ///
  /// Performs no system change. Returns `null` when no installation is offered.
  Future<RuntimeInstallPlanResponse> planRuntimeInstall() {
    return Future.error(
      UnsupportedError('Runtime installation is not supported by this client.'),
    );
  }

  /// Records an explicit decision for one exact plan revision.
  Future<RuntimeInstallJobSnapshot> decideRuntimeInstall(
    RuntimeInstallJobId jobId,
    int planRevision,
    RuntimeInstallApprovalDecision decision,
  ) {
    return Future.error(
      UnsupportedError('Runtime installation is not supported by this client.'),
    );
  }

  /// Starts approved installation work.
  Future<RuntimeInstallJobSnapshot> startRuntimeInstall(
    RuntimeInstallJobId jobId,
  ) {
    return Future.error(
      UnsupportedError('Runtime installation is not supported by this client.'),
    );
  }

  /// Reads authoritative durable installation state.
  Future<RuntimeInstallJobSnapshot?> checkRuntimeInstall(
    RuntimeInstallJobId jobId,
  ) {
    return Future.error(
      UnsupportedError('Runtime installation is not supported by this client.'),
    );
  }

  /// Withdraws a previous untested-version acknowledgement.
  Future<RuntimeHealthReport> revokeUntestedRuntimeVersion() {
    return Future.error(
      UnsupportedError('Runtime consent is not supported by this client.'),
    );
  }

  Future<CoreOperation> startRuntimeOperation(RuntimeOperationKind kind) {
    return Future.error(
      UnsupportedError(
        'Runtime lifecycle operations are not supported by this client.',
      ),
    );
  }

  Stream<RuntimeOperationEvent> observeRuntimeOperation(
    CoreOperation operation,
  ) {
    return Stream.error(
      UnsupportedError(
        'Runtime lifecycle events are not supported by this client.',
      ),
    );
  }

  Future<bool> cancelRuntimeOperation(CoreOperation operation) {
    return Future.error(
      UnsupportedError(
        'Runtime lifecycle cancellation is not supported by this client.',
      ),
    );
  }

  Future<RuntimeModelInventory> listRuntimeModels() {
    return Future.error(
      UnsupportedError(
        'Runtime model inventory is not supported by this client.',
      ),
    );
  }

  Future<SetupJobSnapshot> createSetupPlan(
    RecommendationPlan recommendation, {
    SetupDestinationCategory destination =
        SetupDestinationCategory.providerManaged,
  }) {
    return Future.error(
      UnsupportedError('Model setup planning is not supported by this client.'),
    );
  }

  Future<SetupJobSnapshot?> recoverSetupJob() {
    return Future.error(
      UnsupportedError('Model setup recovery is not supported by this client.'),
    );
  }

  Future<SetupJobSnapshot> decideSetupApproval(
    SetupJobSnapshot job,
    SetupApprovalDecision decision,
  ) {
    return Future.error(
      UnsupportedError('Model setup approval is not supported by this client.'),
    );
  }

  Future<SetupJobSnapshot> startSetupJob(SetupJobSnapshot job) {
    return Future.error(
      UnsupportedError('Model setup is not supported by this client.'),
    );
  }

  Future<SetupJobSnapshot> setupJobStatus(SetupJobId jobId) {
    return Future.error(
      UnsupportedError('Model setup status is not supported by this client.'),
    );
  }

  Stream<SetupJobEvent> observeSetupJob(
    SetupJobId jobId, {
    required int afterSequence,
  }) {
    return Stream.error(
      UnsupportedError('Model setup events are not supported by this client.'),
    );
  }

  Future<SetupJobSnapshot> cancelSetupJob(SetupJobId jobId) {
    return Future.error(
      UnsupportedError(
        'Model setup cancellation is not supported by this client.',
      ),
    );
  }

  Future<SetupJobSnapshot> retrySetupJob(SetupJobSnapshot job) {
    return Future.error(
      UnsupportedError('Model setup retry is not supported by this client.'),
    );
  }

  Future<ChatRuntimeStatus> chatRuntimeStatus({
    ConversationId? conversationId,
  }) {
    return Future.error(
      UnsupportedError('Local chat is not supported by this client.'),
    );
  }

  Future<ConversationSnapshot> createConversation({String? title}) {
    return Future.error(
      UnsupportedError('Local chat is not supported by this client.'),
    );
  }

  Future<ListConversationsResponse> listConversations({int limit = 50}) {
    return Future.error(
      UnsupportedError('Local chat is not supported by this client.'),
    );
  }

  Future<ConversationSnapshot> getConversation(ConversationId conversationId) {
    return Future.error(
      UnsupportedError('Local chat is not supported by this client.'),
    );
  }

  Future<ConversationSnapshot> renameConversation(
    ConversationId conversationId,
    String title,
  ) {
    return Future.error(
      UnsupportedError('Local chat is not supported by this client.'),
    );
  }

  Future<DeleteConversationResponse> deleteConversation(
    ConversationId conversationId,
  ) {
    return Future.error(
      UnsupportedError('Local chat is not supported by this client.'),
    );
  }

  Future<SendMessageResponse> sendChatMessage(
    ConversationId conversationId,
    String content,
  ) {
    return Future.error(
      UnsupportedError('Local chat is not supported by this client.'),
    );
  }

  Stream<ChatGenerationEvent> observeGeneration(
    GenerationId generationId, {
    int afterSequence = 0,
  }) {
    return Stream.error(
      UnsupportedError('Local chat is not supported by this client.'),
    );
  }

  Future<bool> cancelGeneration(GenerationId generationId) {
    return Future.error(
      UnsupportedError('Local chat is not supported by this client.'),
    );
  }

  Future<void> shutdown() async {}
}

class DisconnectedCoreClient extends CoreClient {
  const DisconnectedCoreClient();

  @override
  Future<CoreConnectionSnapshot> checkConnection() async {
    return const CoreConnectionSnapshot(
      kind: CoreConnectionKind.unavailable,
      issue: CoreConnectionIssue.coreUnavailable,
      diagnosticCode: 'CORE_NOT_CONNECTED',
    );
  }
}
