# Kịch bản trình bày chi tiết các slide SAGE

> Tài liệu này là **speaker notes** đi kèm bộ slide SAGE. Nội dung ưu tiên tiếng Việt; thuật ngữ tiếng Anh được giữ lại trong ngoặc để người trình bày có thể đối chiếu với chữ trên slide.
>
> **Cách sử dụng:** Không cần đọc nguyên văn toàn bộ. Phần **Kịch bản trình bày** là lời nói mẫu; phần **Giải nghĩa** dùng để chuẩn bị trả lời câu hỏi; phần **Cần sửa trên slide** phải được áp dụng trước khi trình chiếu.

---

## 0. Mạch kể chuyện tổng thể

| Giai đoạn | Slide | Câu hỏi mà khán giả được dẫn dắt |
|---|---:|---|
| Định vị | 1 | SAGE nghiên cứu vấn đề gì? |
| Khái niệm nền | 2 | Public và permissioned blockchain khác nhau ở đâu? |
| Động cơ sử dụng | 3–4 | Vì sao tổ chức chọn permissioned blockchain và phải đánh đổi gì? |
| Vấn đề nghiên cứu | 5 | Vì sao consensus ban đầu có thể lỗi thời? |
| Khoảng trống | 6 | Các cách migration truyền thống còn thiếu gì? |
| Giải pháp | 7 | SAGE bảo vệ quá trình chuyển đổi bằng những cơ chế nào? |
| Phương pháp | 8 | Các tuyên bố của SAGE được kiểm tra như thế nào? |
| Kết quả | 9–11 | SAGE thể hiện safety và performance ra sao? |
| Định vị đóng góp | 12 | SAGE mới ở tổ hợp capability nào? |

**Một câu xuyên suốt bài nói:**

> “Permissioned blockchain có thể tồn tại lâu hơn consensus assumptions ban đầu. SAGE biến việc thay consensus engine từ một thao tác vận hành theo lịch thành một distributed protocol có state evidence, migration quorum và rollback boundary rõ ràng.”

---

## 1. Từ điển thuật ngữ dùng xuyên suốt

| Thuật ngữ trên slide | Cách hiểu bằng tiếng Việt |
|---|---|
| Blockchain | Sổ cái phân tán; nhiều nút cùng duy trì và thống nhất một lịch sử giao dịch |
| Public/permissionless blockchain | Blockchain mở; việc kiểm chứng và tham gia consensus không phụ thuộc danh sách tổ chức được cấp quyền trước |
| Permissioned blockchain | Blockchain có cấp quyền; validator hoặc participant được xác định và quản lý theo policy |
| Consensus | Đồng thuận: cơ chế giúp các nút thống nhất block, transaction order và trạng thái chính thức |
| Consensus engine | Bộ máy triển khai thuật toán đồng thuận |
| Legacy engine | Bộ máy đồng thuận cũ đang có authority |
| Target engine | Bộ máy đồng thuận mới dự kiến tiếp quản |
| Validator | Nút tham gia kiểm tra, biểu quyết hoặc finalise block |
| Validator set/committee | Tập validator có quyền tham gia một quyết định |
| Canonical history | Lịch sử chính thức mà hệ thống công nhận |
| Finalise/finality | Chốt block là chính thức; tính chung cuộc của block |
| Finalisation authority | Quyền quyết định block nào trở thành chính thức |
| Fork | Hai lịch sử hoặc hai trạng thái xung đột cùng được các nhóm khác nhau coi là chính thức |
| Network partition | Mạng bị chia thành các nhóm tạm thời không liên lạc được; partition không nhất thiết gây fork |
| Quorum | Số lượng tối thiểu validator phải đồng ý để một quyết định hợp lệ |
| `n` | Tổng số validator trong migration committee |
| `f` | Số validator lỗi hoặc Byzantine tối đa nằm trong giả định thiết kế |
| `n-f` | Số validator tối thiểu SAGE yêu cầu cùng xác nhận migration boundary |
| Byzantine fault | Nút có thể gian lận, thông đồng hoặc gửi thông tin mâu thuẫn |
| Crash fault | Nút dừng hoặc im lặng nhưng không cố gửi thông tin gian lận |
| State root | Giá trị băm cam kết toàn bộ trạng thái sau khi thực thi block |
| Shadow execution | Target tái thực thi canonical block nhưng chưa được quyền finalise |
| Shadow anchoring | Neo kết quả target vào canonical state root của legacy chain |
| Boundary | Điểm ranh giới nơi authority được chuyển từ legacy sang target |
| Attestation | Lời xác nhận của validator về cùng một boundary tuple |
| Cutover | Thời điểm chuyển authority sang target engine |
| Provisional | Tạm thời/chưa được seal tuyệt đối |
| Provisional suffix | Chuỗi block target tạo sau cutover nhưng vẫn còn trong rollback window |
| Rollback | Quay về certified anchor và bỏ provisional suffix |
| Bounded rollback | Rollback chỉ được phép trong phạm vi và thời hạn xác định |
| Fail-closed | Khi thiếu bằng chứng hoặc context, từ chối hành động thay vì đoán và tiếp tục nguy hiểm |
| Throughput | Thông lượng; số giao dịch được commit trong một giây, thường đo bằng TPS |
| Confidence interval | Khoảng tin cậy; thể hiện mức bất định do số lần thử hữu hạn |
| Wilson 95% interval | Phương pháp tính khoảng tin cậy cho tỷ lệ nhị phân như fork/không fork |
| Evidence tier | Tầng bằng chứng: model, simulation, process hoặc independent host |
| Overclaim | Tuyên bố rộng hoặc mạnh hơn những gì dữ liệu thực sự chứng minh |

---

# Slide 1 — Trang bìa SAGE

## Nội dung trên slide

**SAGE — No-Scheduled-Halt Migration Between Heterogeneous Consensus Engines for Permissioned Blockchains**

## Giải nghĩa tiêu đề

- **SAGE**: *Shadow-Anchored Graceful Evolution*.
- **Shadow-Anchored**: target engine chạy ở chế độ shadow và kết quả của nó được đối chiếu với canonical state của legacy chain.
- **Graceful Evolution**: quá trình tiến hóa có kiểm soát; authority không được bật bằng local flag tùy ý.
- **No-scheduled-halt**: migration thành công không cần một khoảng dừng được lên lịch trước.
- **Không đồng nghĩa zero downtime tuyệt đối**: khi không đạt quorum, migration có thể bị trì hoãn hoặc stall để giữ safety.
- **Heterogeneous consensus engines**: hai engine có thể khác fault model, quorum rule, certificate và finality semantics; ví dụ PoA/CFT sang BFT.

## Kịch bản trình bày

> “Kính chào thầy cô và các bạn. Hôm nay tôi trình bày SAGE, viết tắt của Shadow-Anchored Graceful Evolution. Bài toán chúng tôi quan tâm là: một permissioned blockchain đang vận hành có thể thay consensus engine như thế nào mà không cần lên lịch dừng toàn bộ hệ thống, đồng thời không tạo ra hai lịch sử xung đột tại điểm chuyển đổi?”

> “Từ khóa quan trọng nhất trong tiêu đề là heterogeneous. Đây không chỉ là đổi hai implementation của cùng một BFT family. Source và target có thể khác trust assumption, quorum rule và cách tạo finality, chẳng hạn từ PoA hoặc crash-oriented engine sang Byzantine fault-tolerant engine.”

> “SAGE không tuyên bố hệ thống luôn có availability trong mọi partition. No-scheduled-halt có nghĩa happy-path migration không đòi planned shutdown. Nếu evidence hoặc quorum chưa đủ, SAGE chọn fail-closed: chưa chuyển authority thay vì cố chuyển và tạo fork.”

> “Để hiểu tại sao bài toán này quan trọng, trước hết chúng ta cần phân biệt public blockchain và permissioned blockchain.”

## Thông điệp cần nhớ

> SAGE bảo vệ **điểm chuyển quyền finalise**, không phải chỉ tự động hóa việc triển khai phần mềm mới.

## Cần sửa trên slide

- Nếu có số `1` rời phía trên footer, hãy xóa vì đã có số trang `01`.
- Chuẩn hóa `VNU HCM` thành `VNU-HCM`.
- Có thể thêm một dòng nhỏ: `SAGE = Shadow-Anchored Graceful Evolution`.
- Không đổi subtitle thành `Zero-Downtime Migration`, vì claim đó mạnh hơn evidence.

---

# Slide 2 — Public blockchain và Permissioned blockchain

## Mục tiêu

Giúp người nghe hiểu hai mô hình khác nhau ở **quyền tham gia, Sybil resistance và governance**, không chỉ khác ở việc dữ liệu công khai hay riêng tư.

## Giải thích Public blockchain

- Bất kỳ ai thường có thể đọc và kiểm chứng ledger.
- Việc tham gia consensus dựa trên điều kiện mở hoặc điều kiện kinh tế.
- **Bitcoin** dùng Proof of Work: người tham gia cạnh tranh bằng computation và energy.
- **Ethereum** dùng Proof of Stake: validator đặt stake và chịu economic penalty.
- Các logo khác chỉ nên được dùng như ví dụ minh họa; nên thêm tên chữ dưới logo để khán giả không phải đoán.
- Điểm mạnh: public verifiability, open participation và censorship resistance.
- Đánh đổi: privacy, fee predictability, performance và governance coordination có thể khó hơn.

## Giải thích Permissioned blockchain

- Validator/operator được nhận diện và cấp quyền.
- Sybil resistance dựa vào identity, PKI, membership policy hoặc legal agreement.
- **Hyperledger Fabric**: dùng membership service, endorsement policy và ordering service.
- **Hyperledger Besu**: Ethereum client có thể triển khai permissioned network với PoA/QBFT-style consensus.
- Permissioned không nhất thiết private hoàn toàn: có thể public-read nhưng chỉ validator được cấp quyền mới finalise.

## Kịch bản trình bày

> “Blockchain không chỉ có một mô hình. Ở bên trái là public hoặc permissionless blockchain. Các hệ thống như Bitcoin và Ethereum cho phép bất kỳ ai kiểm chứng ledger; quyền tham gia consensus không phụ thuộc một danh sách doanh nghiệp được phê duyệt trước.”

> “Public blockchain phải giải bài toán Sybil: một người có thể tạo hàng nghìn identity giả. Bitcoin dùng chi phí tính toán; Ethereum dùng stake và hình phạt kinh tế. Nhờ đó, hệ thống không cần biết trước validator là tổ chức nào.”

> “Ở bên phải là permissioned blockchain. Tại đây, validator đã được nhận diện và cấp quyền bằng membership policy, PKI hoặc consortium governance. Hyperledger Fabric và permissioned Besu là các ví dụ thường gặp.”

> “Điểm quan trọng là permissioned không có nghĩa mọi dữ liệu đều bí mật, và public không có nghĩa mọi người đều có quyền finalise. Chúng ta nên tách bốn quyền: quyền đọc, gửi transaction, validate và thay đổi protocol.”

> “Public blockchain tối ưu open participation và censorship resistance. Permissioned blockchain tối ưu deterministic finality, privacy, predictable operation và accountability. Không mô hình nào tốt hơn trong mọi use case.”

> “Vậy tại sao nhiều tổ chức chấp nhận permissioning? Slide tiếp theo trả lời bằng các use case cụ thể.”

## Thông điệp cần nhớ

> Permissioned blockchain không loại bỏ trust; nó chuyển trust từ anonymous economic competition sang identity, quorum và governance.

## Cần sửa trên slide

- Thêm nhãn chữ dưới từng logo; không chỉ để logo.
- Nếu logo public không đúng với tên hệ thống dự kiến, thay bằng ba ví dụ rõ: `Bitcoin`, `Ethereum`, `Solana`.
- Dùng tiêu đề `Permissioned blockchain / DLT` nếu đưa Corda vào các slide sau.

---

# Slide 3 — Vì sao tổ chức ưa chuộng Permissioned Blockchain?

## Giải nghĩa bảng use case

### Supply-chain

- **Parties**: nhà sản xuất, logistics, hải quan, retailer.
- **Provenance**: nguồn gốc và lịch sử di chuyển của hàng hóa.
- Lợi ích của shared ledger: nhiều tổ chức cùng ghi và audit mà không giao toàn quyền sửa lịch sử cho một bên.
- Fabric-based consortium là kiểu triển khai, không phải bằng chứng mọi supply chain đều cần blockchain.

### Interbank settlement

- **Settlement**: thanh toán bù trừ cuối cùng giữa các ngân hàng.
- **Clearing member**: thành viên tham gia bù trừ nghĩa vụ.
- **Regulator**: cơ quan quản lý.
- **Shared finality**: các pháp nhân độc lập cùng công nhận một trạng thái thanh toán cuối cùng.
- Corda là permissioned DLT; permissioned EVM là mạng tương thích Ethereum nhưng giới hạn validator.

### Enterprise asset/token network

- **Issuer**: tổ chức phát hành tài sản/token.
- **Custodian**: đơn vị lưu ký.
- **Operator**: đơn vị vận hành mạng.
- **Auditor**: bên kiểm toán.
- **Shared ownership state**: trạng thái sở hữu chung mà các bên cùng xác nhận.
- **Programmable policy**: quy tắc chuyển nhượng hoặc compliance được thực thi bằng logic chương trình.

## Khi nào không nên dùng blockchain?

Nếu một tổ chức duy nhất có authority và mọi bên đều tin tổ chức đó, database tập trung thường đơn giản, nhanh và rẻ hơn. Permissioned ledger phù hợp khi nhiều tổ chức độc lập cần shared state nhưng không muốn một bên đơn phương sửa lịch sử.

## Kịch bản trình bày

> “Slide này trả lời câu hỏi: tại sao không dùng một database tập trung? Câu trả lời là permissioned ledger chỉ hợp lý khi có nhiều tổ chức độc lập cùng cần ghi và kiểm chứng một shared history.”

> “Trong supply chain, nhà sản xuất, đơn vị logistics, hải quan và retailer cần theo dõi provenance. Nếu một bên duy nhất sở hữu database, các bên còn lại phải tin bên đó không sửa dữ liệu. Consortium ledger phân phối quyền ghi và audit theo policy.”

> “Trong interbank settlement, các ngân hàng là các pháp nhân độc lập. Shared finality có nghĩa họ cùng công nhận một trạng thái nghĩa vụ cuối cùng, thay vì mỗi ngân hàng giữ một phiên bản khác.”

> “Trong enterprise asset network, issuer, custodian, operator và auditor cần cùng quan sát ownership state và policy. Permissioned EVM hoặc QBFT-style deployment có thể cung cấp deterministic finality và access control.”

> “Tuy nhiên, tôi không nói blockchain luôn tốt hơn database. Nếu chỉ có một authority được tất cả tin tưởng, database là lựa chọn hợp lý hơn. Blockchain có ý nghĩa khi shared governance và resistance to unilateral rewrite thực sự cần thiết.”

> “Những lợi ích này đi cùng một cái giá. Slide tiếp theo trình bày các đánh đổi của permissioned model.”

## Cần sửa trên slide

- Sửa `Permissiond Blockchain` thành `Permissioned Blockchain`.
- Bỏ khoảng trắng trước dấu hỏi: `Blockchain?`.
- Có thể đổi header tiếng Anh thành tiếng Việt:
  - `Use case` → `Trường hợp sử dụng`.
  - `Parties` → `Các bên tham gia`.
  - `Reason` → `Lý do dùng sổ cái chung`.
  - `Case Study` → `Nền tảng/bối cảnh ví dụ`.
- Không gọi các nền tảng là case study nếu không trình bày một deployment cụ thể; dùng `Ví dụ nền tảng` chính xác hơn.

---

# Slide 4 — Lợi ích và đánh đổi của Permissioned Blockchain

## Giải thích từng hàng

### Fast deterministic finality

- Block được chốt nhanh và không dựa vào confirmation depth dài.
- Đổi lại, guarantee phụ thuộc vào validator identity, quorum threshold và fault assumption.

### Controlled privacy/access

- Có thể giới hạn ai đọc, gửi hoặc xác nhận dữ liệu.
- Đổi lại, key management, membership change và policy lifecycle trở nên phức tạp.

### Predictable fee/performance

- Không có public fee auction ở cùng mức như permissionless chain; workload và validator hardware dễ kiểm soát hơn.
- Đổi lại, open competition và decentralization thường thấp hơn; governance tập trung hơn.

### Custom consensus choice

- Consortium có thể chọn PoA, Raft, PBFT, QBFT hoặc engine phù hợp.
- Đổi lại, engine phù hợp lúc khởi tạo có thể lỗi thời khi validator set và threat model thay đổi.

## Kịch bản trình bày

> “Permissioned blockchain không miễn phí về mặt kiến trúc. Mỗi lợi ích ở cột trái tạo ra một trách nhiệm dài hạn ở cột phải.”

> “Fast deterministic finality rất hấp dẫn cho settlement, nhưng nó chỉ đúng khi validator identities và quorum configuration được quản lý đúng. Một threshold sai có thể biến tốc độ thành safety risk.”

> “Controlled privacy giúp đáp ứng compliance, nhưng hệ thống phải xử lý vòng đời của key, certificate, membership và access policy. Khi một tổ chức rời consortium, quyền của họ phải bị thu hồi nhất quán.”

> “Predictable performance đến từ việc kiểm soát participant và infrastructure. Đổi lại, governance tập trung hơn và khả năng chống kiểm duyệt không giống public blockchain.”

> “Cuối cùng, custom consensus choice là lợi ích trực tiếp dẫn đến đề tài này. Engine có thể phù hợp ở ngày đầu nhưng không còn phù hợp sau nhiều năm. Khi đó, consortium phải thay consensus mà không làm mất shared history.”

> “Slide tiếp theo cho thấy lifecycle pressure này phát triển như thế nào và SAGE bước vào ở đâu.”

## Cần sửa trên slide

- Sửa `Permissiond` thành `Permissioned`.
- Có thể đổi `Fast deterministic finality` thành `Tính chung cuộc nhanh và xác định`.
- Không nói predictable performance là guaranteed performance; đây là khả năng dự đoán tốt hơn trong controlled deployment.

---

# Slide 5 — Từ use case đến lifecycle problem và đề xuất SAGE

## Giải thích các bullet

- **Consortium mở rộng**: thêm tổ chức và validator mới; trust giữa các bên có thể giảm.
- **Validator set thay đổi**: committee không còn nhỏ hoặc ổn định như ban đầu.
- **Threat model chuyển đổi**: ban đầu chỉ lo crash hoặc tin authority trung thực; sau đó phải xét equivocation, collusion và Byzantine behavior.
- **Compliance tăng**: yêu cầu audit, key custody, access control và accountability nghiêm ngặt hơn.
- **Throughput tăng**: số giao dịch mỗi giây cần xử lý cao hơn.
- **Finality requirement nghiêm ngặt hơn**: thời gian chốt và khả năng không đảo ngược phải rõ hơn.
- **Consensus engine lỗi thời**: assumption và performance envelope ban đầu không còn phù hợp.

## Kịch bản trình bày

> “Một permissioned network không đứng yên. Consortium mở rộng, validator set thay đổi và các operator trở nên độc lập hơn. Vì vậy trust assumption ngày đầu có thể không còn đúng.”

> “Ví dụ, mạng bắt đầu với sáu authority dùng PoA vì các bên quen biết và deployment cần đơn giản. Sau vài năm, số tổ chức tăng, giá trị tài sản lớn hơn và hệ thống phải chịu được một validator gửi thông tin mâu thuẫn. Threat model đã chuyển từ crash hoặc honest-authority assumption sang Byzantine concern.”

> “Cùng lúc đó, compliance, throughput và finality requirements trở nên nghiêm ngặt hơn. Consensus engine ban đầu có thể không còn đáp ứng được, nhưng ledger chứa shared commitments nên không thể reset.”

> “SAGE được đề xuất cho chính lifecycle problem này: cho phép permissioned blockchain tiến hóa trust model và consensus engine, đồng thời duy trì tính liên tục, bất biến và khả năng kiểm chứng của canonical history.”

> “Cần nhấn mạnh: SAGE không chỉ sao lưu dữ liệu rồi khởi động chain mới. Nó phải chuyển quyền quyết định block chính thức từ legacy engine sang target engine.”

> “Trước khi đi vào kiến trúc, chúng ta xem các cách migration truyền thống và phần còn thiếu của chúng.”

## Cần sửa trên slide

- Sửa `Consortiuum` thành `Consortium` nếu ảnh gốc đang viết sai.
- Thay `Byzantine concerns` bằng `Byzantine faults/behavior` hoặc `lỗi/hành vi Byzantine`.
- Thay `bảo toàn trọn vẹn lịch sử dữ liệu` bằng `duy trì tính liên tục và khả năng kiểm chứng của lịch sử sổ cái`; cách cũ dễ bị hiểu là bảo đảm mọi dữ liệu ngoài chuỗi.
- Khai triển `SAGE = Shadow-Anchored Graceful Evolution` khi xuất hiện lần đầu.

---

# Slide 6 — Trước SAGE, permissioned blockchain migrate như thế nào?

## Giải thích các cách tiếp cận

### Stop-the-world

- Tất cả validator dừng tại một height đã định.
- Operator thay implementation/configuration rồi restart.
- Ưu điểm: boundary dễ quan sát.
- Nhược điểm: planned downtime và coordinated restart risk.
- Rollback thường thủ công từ snapshot hoặc backup.

### Flag-day hard fork

- Mỗi node được cấu hình đổi rule ở một height hoặc thời điểm định trước.
- Không bắt buộc planned halt.
- Nếu node rollout lệch hoặc mạng partition, hai nhóm có thể áp dụng rule khác nhau và tạo fork.
- “Rollback” thường thực chất là tổ chức thêm một fork hoặc coordinated recovery.

### Membership reconfiguration

- Thay validator set hoặc configuration trong cùng consensus protocol.
- Ví dụ joint consensus hoặc epoch-based committee change.
- Không giải quyết đầy đủ việc thay chính finality engine.

### Forkless runtime upgrade

- Thay execution/runtime logic bằng on-chain governance hoặc activation rule.
- Thường giữ nguyên finality gadget.
- Vì vậy không tương đương heterogeneous consensus migration.

### SAGE

- Happy path không yêu cầu scheduled halt.
- Có thể đổi heterogeneous consensus engine.
- Boundary được bảo vệ bằng shadow evidence và migration quorum.
- Rollback chỉ có điều kiện trong provisional window; không phải “luôn đảm bảo”.

## Kịch bản trình bày

> “Trước SAGE, operator có bốn nhóm lựa chọn chính. Stop-the-world dễ kiểm soát nhất: dừng mọi validator, chụp snapshot, thay engine và restart. Nhưng nó tạo downtime và một recovery cliff nếu restart không thành công.”

> “Flag-day hard fork tránh planned stop bằng cách yêu cầu mọi node đổi rule tại cùng height. Vấn đề là local height không phải distributed agreement. Trong partition, hai nhóm có thể cùng tin rằng mình đã đến flag day và tiếp tục trên hai history.”

> “Membership reconfiguration rất mạnh khi đổi ai tham gia consensus, nhưng vẫn giữ cùng protocol family và finality rule. Forkless runtime upgrade thay logic thực thi nhưng thường không thay finality engine.”

> “SAGE nhắm đúng khoảng trống còn lại: thay consensus authority giữa hai engine khác assumption, trong khi target được chuẩn bị song song và cutover chỉ xảy ra khi đủ evidence.”

> “Rollback của SAGE phải được mô tả chính xác: chỉ target-only provisional suffix có thể bị bỏ trước deadline và khi context hợp lệ. Sau seal, absolute history không được đảo.”

> “Slide tiếp theo mở SAGE thành bốn cơ chế bảo vệ nối tiếp nhau.”

## Cần sửa bắt buộc trên slide

Thay hàng SAGE:

| Ô hiện tại | Nội dung nên dùng |
|---|---|
| `Không dừng` | `Không scheduled halt khi handoff thành công` |
| `Có` | `Có — heterogeneous engine` |
| `Có thể và luôn đảm bảo` | `Có điều kiện — bounded, fail-closed` |

**Không được nói:** “SAGE luôn rollback thành công.” Nếu context thiếu, bị sửa hoặc deadline đã qua, rollback phải bị từ chối.

---

# Slide 7 — Kiến trúc đề xuất của SAGE

## Luồng tổng thể

```text
Legacy finalises canonical blocks
        ↓
Target shadow-executes cùng blocks
        ↓
κ state-root matches tạo readiness evidence
        ↓
Đủ n-f attestation cho cùng boundary
        ↓
Authority chuyển sang target
        ↓
Target suffix provisional đến rollback deadline
```

## Hàng 1 — Single-finalizer dual-run

- **Input**: canonical legacy block và target validator.
- **Dual-run**: hai engine cùng hoạt động.
- **Single-finalizer**: chỉ legacy engine được tạo canonical finality trước cutover.
- Target không có finalisation authority.
- Kết quả: tránh hai finalizer đồng thời.

## Hàng 2 — Shadow anchoring

- **Canonical root**: state root nằm trong block legacy đã finalise.
- **Recomputed root**: root target tự tính lại sau khi thực thi cùng block.
- **`κ` lần khớp liên tiếp**: target phải match nhiều block liên tiếp, không chỉ một block.
- Nếu mismatch, readiness bị reset hoặc migration bị từ chối.
- Output là readiness evidence, chưa phải authority.

## Hàng 3 — Migration quorum

- **Attestation**: validator xác nhận target đã ready tại một boundary cụ thể.
- Tất cả attestation phải bind cùng chain, epoch, configuration, height, boundary block/root và target engine.
- **`n`**: tổng validator trong migration committee.
- **`f`**: số validator lỗi/Byzantine tối đa trong giả định.
- **`n-f`**: threshold SAGE chọn cho handoff.
- Output: cho phép chuyển finalisation authority.

### Ví dụ `n=6`, `f=1`


a) Threshold:

\[
q=n-f=6-1=5
\]

b) Partition `3/3`:

- Nhóm A có 3 validator.
- Nhóm B có 3 validator.
- `3 < 5`, nên không nhóm nào được cutover.
- Migration có thể stall nhưng không sinh hai target histories do hai bên tự chuyển.

## Hàng 4 — Bounded rollback

- **Anchor**: certified legacy-finalised boundary.
- **Provisional suffix**: target-only blocks sau cutover nhưng trước seal.
- **Abort before deadline**: rollback request phải đến trong rollback window.
- **Context hợp lệ**: dữ liệu replay như timestamp, beneficiary, randomness hoặc oracle snapshot phải đầy đủ và khớp commitment.
- **Fail-closed**: context thiếu/sai thì từ chối rollback.

## Kịch bản trình bày

> “SAGE gồm bốn lớp bảo vệ nối tiếp. Hai lớp đầu trả lời target đã thực thi đúng hay chưa; lớp thứ ba trả lời target đã được phép nhận authority chưa; lớp cuối giới hạn recovery nếu target gặp lỗi.”

> “Đầu tiên là single-finalizer dual-run. Legacy và target cùng chạy, nhưng chỉ legacy được finalise. Điều này cho phép quan sát target sớm mà không tạo hai nguồn canonical history.”

> “Thứ hai là shadow anchoring. Target tái thực thi canonical block và so recomputed state root với canonical root. Hệ thống yêu cầu kappa lần match liên tiếp. Kappa là tham số readiness, không phải quorum.”

> “Thứ ba là migration quorum. `n` là tổng validator trong committee; `f` là số validator lỗi tối đa nằm trong giả định. SAGE yêu cầu `n-f` validator khác nhau xác nhận cùng boundary. Với `n=6`, `f=1`, cần năm xác nhận.”

> “Nếu mạng bị chia ba–ba, mỗi bên chỉ có ba validator, không đạt năm. SAGE không cố chọn một bên bằng local clock; nó chưa chuyển authority. Đây là fail-closed behavior.”

> “Cuối cùng là bounded rollback. Sau cutover, target suffix còn provisional. Nếu abort đến trước deadline và replay context hợp lệ, suffix có thể bị bỏ. Sau seal hoặc khi context sai, rollback bị từ chối.”

> “Điểm cốt lõi là readiness không đồng nghĩa authority, và rollback không đồng nghĩa đảo mọi finalised history.”

## Cần sửa trên slide

- Dùng `κ` thay vì chữ `K` nếu tài liệu khoa học định nghĩa tham số là kappa.
- Có thể đổi `Target không có finalise authority` thành `Target chưa có quyền finalise`.
- Có thể đổi `Rollback fail-closed` thành `Rollback có kiểm tra; sai context thì từ chối` để khán giả Việt dễ hiểu.

---

# Slide 8 — Cách thức thực nghiệm

## Vì sao cần nhiều tầng bằng chứng?

Không một phương pháp đơn lẻ đủ để chứng minh toàn bộ hệ thống:

- Type/lint/unit test tìm lỗi implementation cơ bản.
- Formal model tìm counterexample trong state space trừu tượng.
- Simulation cho phép fault injection có kiểm soát.
- Concurrent processes kiểm tra race và partition thực.
- Independent hosts loại bỏ một phần shared-machine artifacts.
- Statistics biểu diễn uncertainty.

## Giải thích từng hàng

### Static quality

- **Type checks**: kiểm tra kiểu dữ liệu.
- **Lint**: phát hiện pattern code rủi ro hoặc không nhất quán.
- **Unit tests**: kiểm tra từng thành phần nhỏ.
- **Invariant checks**: kiểm tra thuộc tính phải luôn đúng.
- Không tự chứng minh distributed safety.

### Formal safety

- **Trace**: một chuỗi trạng thái và sự kiện.
- **Boundary safety**: không có hai correct validator chấp nhận hai migration decisions xung đột.
- **Bounded exhaustive model checking**: duyệt hết các trạng thái trong phạm vi cấu hình hữu hạn.
- Không phải proof cho mọi `n` và mọi network.

### Behavioral conformance

- So sánh reachable outcomes của formal model và executable artifact.
- Mục tiêu: tránh model chứng minh một rule nhưng code chạy rule khác.

### Controlled simulation

- **Fixed seed**: dùng seed cố định để tái tạo campaign.
- **Adversarial campaign**: chủ động inject partition, delay, mismatch hoặc weakened gate.
- Tìm exposure của safety/liveness theo tham số.

### Concurrent execution

- Chạy independent validator processes thực sự song song.
- Kiểm tra fork có xuất hiện khi hai partition cùng tiến hay không.

### Host realism

- Dùng nhiều máy độc lập.
- Network impairment: delay, loss hoặc RTT injection.
- Giảm nguy cơ kết quả chỉ do shared process/clock.

### Statistical review

- Confidence interval: mức bất định.
- Effect size: độ lớn khác biệt.
- Multiplicity control: kiểm soát false positive khi kiểm tra nhiều giả thuyết.
- Với sample nhỏ, phải báo cáo tier riêng thay vì gộp tùy tiện.

## Kịch bản trình bày

> “Để đánh giá một migration protocol, chúng tôi không dựa vào một benchmark duy nhất. Slide này trình bày evidence ladder từ static checks đến independent hosts.”

> “Static checks bảo đảm artifact có chất lượng cơ bản, nhưng không chứng minh distributed safety. Vì vậy bước tiếp theo là bounded model checking để tìm trace phá boundary invariant trong state space hữu hạn.”

> “Sau đó behavioral conformance so kết quả có thể đạt của model với executable artifact. Mục tiêu là tránh trường hợp model đúng nhưng implementation quyết định khác.”

> “Controlled simulation cho phép tái tạo fault bằng fixed seed. Tuy nhiên simulator có thể không đại diện concurrency thật, nên chúng tôi chạy independent validator processes dưới partition.”

> “Tiếp theo, independent cloud hosts giúp kiểm tra liệu kết quả có phụ thuộc shared process, shared clock hay loopback networking không.”

> “Cuối cùng, statistical review báo cáo uncertainty. Khi số lần chạy nhỏ, chúng tôi không chỉ nói 0 phần trăm hoặc 100 phần trăm; chúng tôi kèm confidence interval và sample size.”

> “Ba slide tiếp theo lần lượt trình bày safety differential, evidence tiers và throughput.”

## Cần sửa trên slide

- `Khác biệt có ổn định đúng?` nên đổi thành `Khác biệt có ổn định và có ý nghĩa không?`.
- Có thể Việt hóa header:
  - `Question` → `Câu hỏi`.
  - `Static quality` → `Chất lượng tĩnh`.
  - `Formal safety` → `An toàn hình thức`.
  - `Host realism` → `Độ thực tế ở mức máy độc lập`.
- Không gọi bounded model checking là mathematical proof cho mọi cấu hình.

---

# Slide 9 — Partition safety: SAGE `0/20`, controls `20/20`

## Cách đọc biểu đồ

- Trục dọc: tỷ lệ lần chạy xuất hiện fork.
- Mỗi arm có 20 lần chạy độc lập.
- Scenario: mạng 6 validator bị chia cân bằng `3/3` tại cutover.
- Chấm/thanh thể hiện observed fork rate.
- Whisker đen thể hiện Wilson 95% confidence interval.

## Ba protocol arm

### SAGE `n-f` gate

- `n=6`, `f=1`, nên threshold là 5.
- Mỗi partition chỉ có 3 validator.
- Không bên nào đạt 5, nên không bên nào chuyển authority.
- Quan sát `0/20 forks`.

### Blind hard fork

- Node tự chuyển theo trigger cục bộ, không có same-boundary migration quorum.
- Hai partition có thể cùng tiến và tạo history xung đột.
- Quan sát `20/20 forks`.

### Cox-threshold control `2f+1`

- Đây là **weakened threshold control trong campaign**, không nên mô tả như toàn bộ Cox protocol.
- Với `f=1`, `2f+1=3`.
- Mỗi partition có đúng 3 validator nên cả hai có thể đạt threshold.
- Quan sát `20/20 forks`.

## `0/20` có nghĩa gì?

- Trong 20 lần chạy, không lần nào quan sát thấy fork.
- Observed rate là 0%.
- Không có nghĩa xác suất thực tế tuyệt đối bằng 0.
- Wilson 95% interval vẫn là `0–16.1%`.

## `20/20` có nghĩa gì?

- Cả 20 lần chạy đều quan sát thấy fork.
- Observed rate là 100%.
- Wilson 95% interval là `83.9–100%`.

## Fork là gì?

Fork xảy ra khi hai nhóm finalise hai block hoặc hai state root xung đột tại cùng height:

```text
                → Block h-A → state root A
Block h-1
                → Block h-B → state root B
```

Partition chỉ là mất liên lạc; fork là hậu quả hai lịch sử được coi là chính thức. Một protocol an toàn có thể stall trong partition mà không fork.

## Kịch bản trình bày

> “Đây là safety result quan trọng nhất. Chúng tôi tạo balanced partition ba–ba đúng tại cutover và lặp mỗi arm 20 lần.”

> “Trục dọc là observed fork rate. Fork ở đây nghĩa là hai partition finalise hai state xung đột tại cùng height.”

> “Với SAGE, `n=6`, `f=1`, nên migration gate bằng `n-f=5`. Mỗi partition chỉ có ba validator, không bên nào đạt năm. Vì vậy authority chưa chuyển. Kết quả quan sát là 0 fork trong 20 lần.”

> “Blind hard fork không có distributed boundary gate; cả hai partition có thể tự chuyển theo local trigger. Kết quả là 20 fork trong 20 lần.”

> “Cox-threshold trên hình là một weakened threshold control dùng `2f+1`. Với `f=1`, threshold bằng ba, nên cả hai partition ba validator đều đạt. Nó cũng fork 20 trên 20.”

> “Whiskers là Wilson 95% intervals. Vì sample hữu hạn, 0 trên 20 không chứng minh probability bằng 0. Cách nói đúng là: trong campaign này, không quan sát fork với SAGE, trong khi cả hai unsafe controls fork ở mọi lần chạy.”

> “Kết quả này cho thấy gate thay đổi observed outcome và negative controls chứng minh experiment có khả năng tạo fork khi protection bị loại bỏ.”

## Không được overclaim

- Không nói “SAGE không thể fork trong mọi deployment”.
- Không gọi `2f+1` là toàn bộ Cox protocol.
- Không nói `n-f` là threshold an toàn duy nhất.
- Không nói stall là downtime bằng 0; đây là safety-over-availability choice trong partition.

---

# Slide 10 — Evidence ladder qua ba tầng thực nghiệm

## Cách đọc hình

- Trục ngang: observed fork rate từ 0% đến 100%.
- Chấm xanh: point estimate của SAGE.
- Dấu X đỏ: point estimate của blind hard fork.
- Whisker: Wilson 95% confidence interval.
- Mỗi hàng là một evidence tier riêng; không gộp các sample khác môi trường.

## Ba evidence tier

### Loopback processes

- Validator chạy thành process riêng nhưng trên cùng môi trường máy/loopback.
- SAGE `0/20`; blind hard fork `20/20`.
- Interval không chồng lấn.

### Independent cloud hosts

- Validator chạy trên nhiều máy độc lập.
- SAGE `0/5`; blind hard fork `5/5`.
- Sample nhỏ hơn nên interval rộng hơn.
- Interval vẫn không chồng lấn: SAGE upper `43.4%`, control lower `56.6%`.

### Injected RTT khoảng 104 ms

- Chủ động thêm round-trip latency gần 104 ms.
- SAGE `0/3`; blind hard fork `3/3`.
- Sample rất nhỏ nên intervals rộng và chồng lấn.
- Chỉ nên xem là directional replication, không phải statistical confirmation mạnh.

## Kịch bản trình bày

> “Một phản biện hợp lý là kết quả 0 trên 20 có thể chỉ là artifact của loopback. Vì vậy hình này mở rộng evidence qua ba tier.”

> “Hàng đầu dùng independent validator processes trên loopback. SAGE ở 0 trên 20; blind hard fork ở 20 trên 20.”

> “Hàng thứ hai chuyển sang independent cloud hosts. Hướng khác biệt được tái hiện: SAGE 0 trên 5, control 5 trên 5. Tuy nhiên sample nhỏ hơn nên confidence intervals rộng hơn.”

> “Hàng cuối inject round-trip latency khoảng 104 millisecond. Kết quả point estimate vẫn là 0 trên 3 và 3 trên 3, nhưng hai confidence intervals chồng lấn vì chỉ có ba lần chạy mỗi arm.”

> “Do đó chúng tôi không pool ba tier để tạo một con số lớn giả tạo. Mỗi tier trả lời một objection khác nhau: concurrency, shared-host dependency và network latency.”

> “Cách kết luận trung thực là observed direction được lặp lại qua ba môi trường, nhưng tier n bằng 3 cần thêm repetitions để có statistical confidence mạnh hơn.”

## Không được overclaim

- Không nói ba tier chứng minh geo-distributed production readiness.
- Không cộng thành `0/28` và `28/28` nếu campaign không homogeneous.
- Không bỏ confidence interval khi nói sample `n=3` hoặc `n=5`.

---

# Slide 11 — Safety tăng mạnh, throughput đo được vẫn gần baseline

## Vì sao có hai panel?

Safety campaign và throughput campaign là hai thí nghiệm khác nhau. Không nên nối chúng thành một scatter correlation hoặc gọi chúng là cùng protocol arm.

## Panel trái — Partition safety

- SAGE: 0% observed fork rate.
- Hard fork: 100%.
- Cox-threshold: 100%.
- Cox-threshold là weakened migration-gate control.

## Panel phải — No-fault throughput

- **Committed throughput**: số transaction đã được commit mỗi giây.
- SAGE: khoảng 777 TPS.
- Hard fork: khoảng 790 TPS.
- Cox-style: khoảng 790 TPS.
- Mean spread quan sát khoảng 1.65%.
- Error bars: standard deviation giữa các validator, không nhất thiết là confidence interval của campaign mean.
- Cox-style throughput arm không phải Cox-threshold safety arm.

## Kịch bản trình bày

> “Slide này trả lời câu hỏi: SAGE có đạt safety bằng cách làm throughput sụp đổ hay không? Vì safety và throughput đến từ hai campaign khác nhau, hình được tách thành hai panel.”

> “Panel trái nhắc lại partition safety: SAGE 0% observed forks, trong khi hard fork và Cox-threshold control là 100%.”

> “Panel phải là no-fault throughput campaign. SAGE đạt khoảng 777 committed transactions per second. Hai comparison arms đạt khoảng 790 TPS. Chênh lệch mean quan sát khoảng 1.65%.”

> “Error bars là độ lệch chuẩn giữa validator observations. Chúng không tự động chứng minh statistical equivalence. Vì vậy kết luận đúng không phải ‘SAGE nhanh bằng tuyệt đối’, mà là trong campaign này không quan sát throughput collapse.”

> “Cần phân biệt hai nhãn Cox. Cox-threshold ở panel safety là threshold ablation. Cox-style ở panel throughput là comparison arm khác. Chúng không phải cùng một protocol arm và không nên dùng để suy ra causal trade-off trực tiếp.”

> “Kết hợp hai panel, chúng ta thấy safety differential lớn trong khi measured no-fault throughput vẫn ở gần các baseline của campaign.”

## Không được overclaim

- Không nói throughput equivalence nếu chưa có equivalence test.
- Không nói đây là throughput của production blockchain.
- Không lấy 1.65% làm universal overhead.
- Không nói safety và throughput có quan hệ nhân quả chỉ từ hai panel khác campaign.

---

# Slide 12 — SAGE novelty: tổ hợp capability

## Cách đọc ký hiệu

- **YES**: capability được giải quyết trực tiếp trong scope của approach.
- **PART**: chỉ có một phần, deployment-specific hoặc không phải core contribution.
- **—**: ngoài phạm vi chính; không có nghĩa approach yếu hoặc sai.
- Bảng là capability synthesis, không phải performance ranking.

## Giải thích từng cột

### Live

Có thể chuyển trong khi service tiếp tục hay không. Với SAGE, “live” có điều kiện: không scheduled halt khi handoff thành công; partition có thể làm migration stall.

### Engine swap

Có thay consensus engine/finality rule thực sự hay chỉ đổi membership hoặc runtime.

### Cross-fault boundary

Có nối source và target khác fault model, ví dụ PoA/CFT sang BFT hay không.

### Shadow state

Target có tái thực thi canonical state trước khi có authority hay không.

### Migration quorum

Có quorum rule riêng bảo vệ authority-transfer decision và cùng boundary tuple hay không.

### Bounded rollback

Có thể bỏ target-only provisional history trong cửa sổ giới hạn mà không đảo absolute legacy history hay không.

### Multi-tier evidence

Có kết hợp theorem/model, negative controls, process experiments và host experiments hay không.

## Giải thích từng hàng

### Stop/restart

Có thể thay engine nhưng dùng downtime để kiểm soát boundary. Rollback chủ yếu operator-coordinated.

### Hard fork

Có thể thay rule tại flag day nhưng thiếu distributed same-boundary decision nếu chỉ dựa local activation.

### Reconfiguration

Thay membership/configuration trong cùng protocol; không nhất thiết thay finality engine.

### Merge/forkless

Chứng minh parallel preparation và deterministic activation có thể triển khai, nhưng thường specialized cho platform hoặc migration pair.

### Abstract/Aliph

Đặt nền tảng compositional cho protocol switching/abort-forward trong setting tương ứng; không tập trung PoA-to-BFT bounded reverse rollback.

### Cox

Direct comparison gần với live BFT engine switching; chủ yếu homogeneous BFT family và forward recovery.

### Adaptive policy

Trả lời khi nào nên đổi và chọn engine nào; thường cần safe switching primitive ở bên dưới.

### SAGE

Nhắm tổ hợp: live conditional handoff, engine swap, cross-fault boundary, shadow state, migration quorum, bounded rollback và evidence tiers.

## Kịch bản trình bày

> “Hình cuối trả lời câu hỏi SAGE mới ở đâu. Đây không phải bảng xếp hạng quality hoặc performance. Mỗi hàng giải một scope khác nhau.”

> “YES nghĩa capability được xử lý trực tiếp. PART nghĩa có một phần hoặc chỉ đúng trong specialized deployment. Dấu gạch ngang nghĩa ngoài core scope, không phải công trình đó kém.”

> “Stop/restart kiểm soát boundary bằng downtime. Hard fork cho activation nhưng có thể thiếu same-boundary quorum. Reconfiguration thay ai tham gia protocol. Merge hoặc forkless upgrade cung cấp specialized transition. Abstract/Aliph và Cox là nền tảng quan trọng cho protocol switching. Adaptive policy trả lời khi nào nên chuyển.”

> “SAGE không claim phát minh live switching. Điểm mới là tổ hợp được viền ở hàng cuối: crossing fault models, shadow state validation, migration-specific quorum và bounded rollback trong cùng một construction.”

> “Cột multi-tier evidence nhấn mạnh cách đánh giá: argument và formal model được nối với negative controls, concurrent processes và independent hosts.”

> “Do đó scientific claim chính xác là: SAGE đề xuất boundary-safety construction cho heterogeneous deterministic-finality migration. Nó không phải consensus engine cạnh tranh TPS với HotStuff, Tendermint hoặc DAG-BFT.”

## Không được overclaim

- Không nói SAGE tốt hơn mọi prior work.
- Không xem số ô YES là điểm chất lượng.
- Không nói capability heatmap là benchmark.
- Không đánh dấu production integration, durable recovery hoặc signed live attestation là hoàn tất nếu chúng vẫn là future work.

---

# 13. Kịch bản kết luận chung

> “Bài trình bày bắt đầu từ sự khác biệt giữa public và permissioned blockchain. Permissioned model mang lại deterministic finality, access control và predictable operation, nhưng tạo một lifecycle responsibility: consortium phải tiến hóa trust model và consensus mà không phá shared history.”

> “Các cách truyền thống hoặc dùng downtime, hoặc dựa vào local activation, hoặc chỉ thay membership/runtime trong cùng finality architecture. SAGE thay đổi cách nhìn: migration boundary phải là một distributed decision.”

> “Construction gồm bốn lớp: legacy là single finalizer trong dual-run; target chứng minh state equivalence bằng shadow anchoring; `n-f` migration quorum trao authority tại cùng boundary; và bounded rollback chỉ cho phép bỏ provisional suffix trước deadline với context hợp lệ.”

> “Experiment quan sát 0 trên 20 forks với SAGE trong balanced partition, trong khi hai unsafe controls là 20 trên 20. Hướng khác biệt được lặp lại qua process, independent-host và injected-RTT tiers. Trong no-fault campaign, throughput của SAGE khoảng 777 TPS so với khoảng 790 TPS ở comparison arms, nên dữ liệu hiện tại không cho thấy throughput collapse.”

> “Kết luận trung thực là SAGE cung cấp evidence rằng migration-specific boundary gate có thể thay đổi observed safety outcome mà không cần scheduled halt ở happy path. Đây vẫn là research prototype; production storage, signed live attestation, key management và geo-distributed validation là các bước tiếp theo.”

---

# 14. Các câu hỏi có thể được hỏi và câu trả lời ngắn

## “Fork là gì?”

> Fork là khi hai nhóm validator cùng coi hai block hoặc hai state khác nhau tại cùng height là chính thức. Network partition chỉ là mất liên lạc; partition chỉ trở thành safety failure khi nó dẫn đến hai canonical histories.

## “`0/20` nghĩa là gì?”

> Trong 20 lần chạy của campaign, không lần nào quan sát thấy fork. Nó không chứng minh xác suất thực tế bằng 0; Wilson 95% interval vẫn là 0 đến 16.1%.

## “`n` và `f` là gì?”

> `n` là tổng validator trong migration committee. `f` là số validator lỗi hoặc Byzantine tối đa nằm trong giả định thiết kế. Với `n=6`, `f=1`, SAGE yêu cầu `n-f=5` validator cùng xác nhận một boundary.

## “Tại sao không dùng `2f+1`?”

> Trong cấu hình dư validator `n=6`, `f=1`, `2f+1=3`. Hai partition ba–ba đều có thể đạt ba. Migration quorum phải được phân tích theo toàn committee và boundary decision, không sao chép máy móc target-engine quorum.

## “SAGE có zero downtime không?”

> Không nên dùng claim tuyệt đối đó. Happy-path migration không cần scheduled halt, nhưng khi không đạt quorum, migration có thể stall để giữ safety.

## “SAGE có luôn rollback được không?”

> Không. Rollback chỉ được phép trước deadline, chỉ bỏ provisional target suffix và yêu cầu context hợp lệ. Sau seal hoặc khi context sai, hệ thống từ chối rollback.

## “Tại sao cần shadow execution?”

> Vì target binary đã chạy hoặc đã sync đến cùng height chưa chứng minh nó tạo cùng state. Shadow execution đối chiếu recomputed root với canonical root trước khi target có authority.

## “SAGE khác Cox ở đâu?”

> SAGE không claim phát minh live switching. Điểm khác chính là heterogeneous fault-model boundary, state-root shadow anchoring, migration-specific `n-f` gate và bounded reverse rollback trong cùng construction.

## “777 TPS có nghĩa SAGE chậm hơn không?”

> Campaign quan sát mean thấp hơn khoảng 1.65%, nhưng chưa đủ để kết luận universal slowdown hoặc statistical non-equivalence. Cách nói đúng là không quan sát throughput collapse trong campaign hiện tại.

---

# 15. Danh sách sửa bắt buộc trước khi trình chiếu

| Slide | Nội dung hiện tại | Nội dung cần sửa |
|---:|---|---|
| 1 | `VNU HCM`, số `1` thừa | `VNU-HCM`; xóa đối tượng số thừa |
| 2–4 | `Permissiond` | `Permissioned` |
| 3 | `Case Study` nhưng chỉ liệt kê platform | Đổi thành `Ví dụ nền tảng/bối cảnh` |
| 5 | `Consortiuum` | `Consortium` |
| 5 | `Byzantine concerns` | `Lỗi/hành vi Byzantine` |
| 5 | `bảo toàn trọn vẹn lịch sử dữ liệu` | `duy trì tính liên tục và khả năng kiểm chứng của lịch sử sổ cái` |
| 6 | SAGE: `Không dừng` | `Không scheduled halt khi handoff thành công` |
| 6 | Rollback: `Có thể và luôn đảm bảo` | `Có điều kiện — bounded, fail-closed` |
| 7 | `K lần khớp` | `κ lần khớp liên tiếp` nếu ký hiệu chuẩn là kappa |
| 8 | `Khác biệt có ổn định đúng?` | `Khác biệt có ổn định và có ý nghĩa không?` |
| 9 | `Cox-threshold` có thể bị hiểu là Cox đầy đủ | Thêm `weakened threshold control` trong caption/notes |
| 11 | Hai nhãn Cox dễ bị đánh đồng | Giữ subtitle nói rõ hai campaign dùng hai controls khác nhau |
| 12 | Heatmap dễ bị xem là ranking | Giữ footer `Capability synthesis — not a performance ranking` |

---

# 16. Checklist trước khi nói

- [ ] Có thể giải thích public và permissioned blockchain mà không nói mô hình nào tốt hơn tuyệt đối.
- [ ] Có thể trả lời khi nào database tốt hơn permissioned ledger.
- [ ] Phân biệt network partition và fork.
- [ ] Giải thích được `n`, `f`, `n-f` bằng ví dụ 6 validator.
- [ ] Phân biệt readiness evidence và finalisation authority.
- [ ] Phân biệt migration quorum và target-engine quorum.
- [ ] Nói đúng ý nghĩa `0/20`, `20/20` và confidence interval.
- [ ] Không đánh đồng Cox-threshold safety control với Cox-style throughput arm.
- [ ] Không nói no-scheduled-halt là zero downtime tuyệt đối.
- [ ] Không nói rollback luôn thành công.
- [ ] Không nói 777 TPS chứng minh universal performance equivalence.
- [ ] Kết thúc bằng scientific contribution, evidence và remaining limitations.
