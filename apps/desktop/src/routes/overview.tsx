import { Button } from '@sonasub/ui/components/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@sonasub/ui/components/card';

export function OverviewPage() {
  return (
    <Card>
      <CardHeader>
        <CardTitle>工程骨架已就绪</CardTitle>
        <CardDescription>
          当前阶段只包含应用壳：路由、主题与共享组件。音频采集、识别与翻译尚未实现。
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-wrap gap-3">
        <Button>主按钮</Button>
        <Button variant="secondary">次按钮</Button>
        <Button variant="outline">描边按钮</Button>
      </CardContent>
    </Card>
  );
}
