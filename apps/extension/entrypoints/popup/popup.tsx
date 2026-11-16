import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@sonasub/ui/components/card';

export function Popup() {
  return (
    <div className="w-80 bg-background p-3 text-foreground">
      <Card>
        <CardHeader>
          <CardTitle>SonaSub</CardTitle>
          <CardDescription>扩展入口已就绪，字幕能力尚未接入。</CardDescription>
        </CardHeader>
        <CardContent className="text-xs text-muted-foreground">
          多标签页处理、采集与识别流程留待后续任务确定。
        </CardContent>
      </Card>
    </div>
  );
}
